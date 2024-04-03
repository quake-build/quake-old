#![feature(let_chains)]
#![allow(dead_code)]

use std::collections::HashMap;
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use nu_parser::parse;
use nu_protocol::ast::{Argument, Block};
use nu_protocol::engine::{EngineState, Stack, StateWorkingSet};
use nu_protocol::{report_error, report_error_new, Span};
use parking_lot::{Mutex, RwLock, RwLockReadGuard};
use tokio::runtime::Runtime;
use tokio::task::{AbortHandle, JoinSet};

use quake_core::metadata::{Metadata, TaskCallId};
use quake_core::prelude::*;
use quake_core::utils::is_dirty;

use crate::nu::eval::{eval_block, eval_task_decl_body, eval_task_run_body};
use crate::nu::parse::parse_metadata;
use crate::nu::{create_engine_state, create_stack};
use crate::run_tree::{generate_run_tree, RunNode};
use crate::state::State;

mod nu;
mod run_tree;
mod state;
mod utils;

#[derive(Debug, Clone)]
pub struct EngineOptions {
    pub quiet: bool,
    pub json: bool,
    pub force: bool,
    pub watch: bool,
}

pub struct Engine {
    project: Project,
    _options: EngineOptions,
    state: Arc<RwLock<State>>,
    engine_state: EngineState,
    stack: Stack,
    task_pool: JoinSet<Result<(TaskCallId, bool), EngineError>>,
    handles: Mutex<HashMap<TaskCallId, (AbortHandle, Arc<AtomicBool>)>>,
}

impl Engine {
    pub fn load(project: Project, options: EngineOptions) -> EngineResult<Self> {
        #[cfg(windows)]
        nu_ansi_term::enable_ansi_support().expect("Failed to initialize ANSI support");

        let state = Arc::new(RwLock::new(State::new()));

        let engine_state = create_engine_state(state.clone());
        let stack = create_stack(project.project_root());

        let mut engine = Self {
            project,
            _options: options,
            state,
            engine_state,
            stack,
            task_pool: JoinSet::new(),
            handles: Mutex::new(HashMap::new()),
        };

        engine.load_script()?;

        Ok(engine)
    }

    fn report_errors(&self, working_set: &StateWorkingSet<'_>) -> bool {
        let mut state = self.state.write();

        if state.errors.is_empty() && working_set.parse_errors.is_empty() {
            return false;
        }

        // report parse errors in working set, but do not discard as the working state
        // is intended to represent such invalid states
        for error in &working_set.parse_errors {
            report_error(working_set, error);
        }

        // report errors emitted by quake, removing them so that the engine may continue
        // to function if recovery is desirable
        for error in state.errors.drain(..) {
            report_error(working_set, &*error);
        }

        true
    }

    fn report_errors_new(&self) -> bool {
        self.report_errors(&StateWorkingSet::new(&self.engine_state))
    }

    fn report_shell_error(&self, error: &ShellError) {
        if error.is_quake_internal() {
            self.report_errors_new();
        } else {
            report_error_new(&self.engine_state, error);
        }
    }

    /// Load and evaluate the project's build script.
    fn load_script(&mut self) -> EngineResult<()> {
        let build_script = self.project.build_script();
        let filename = build_script
            .strip_prefix(self.project.project_root())
            .unwrap_or(build_script)
            .to_string_lossy()
            .into_owned();

        let source = fs::read_to_string(build_script).context("Failed to read build script")?;

        let block = self
            .parse_source(source.as_bytes(), &filename)
            .ok_or_else(|| EngineError::ParseFailed)?;

        match self.eval_block(&block) {
            Ok(true) => {}
            Ok(false) => return Err(EngineError::EvalFailed),
            Err(error) => {
                self.report_shell_error(&error);
                return Err(EngineError::EvalFailed);
            }
        }

        Ok(())
    }

    fn parse_source(&mut self, source: &[u8], filename: &str) -> Option<Block> {
        // parse the build script
        let (block, delta) = {
            let mut working_set = StateWorkingSet::new(&self.engine_state);

            // perform a first-pass parse over the file
            let mut output = parse(&mut working_set, Some(filename), source, false);

            // extract task metadata by reparsing
            parse_metadata(&mut output, &mut working_set, &mut self.state.write());

            // report parsing and internal errors
            if self.report_errors(&working_set) {
                return None;
            }

            (output, working_set.render())
        };

        // merge updated state
        if let Err(err) = self.engine_state.merge_delta(delta) {
            self.report_shell_error(&err);
            return None;
        }

        Some(block)
    }

    /// Evaluate the source of a build file, returning whether or not the
    /// operation completed successfully.
    fn eval_block(&mut self, block: &Block) -> ShellResult<bool> {
        eval_block(block, &self.engine_state, &mut self.stack)
    }

    pub fn project(&self) -> &Project {
        &self.project
    }

    /// Get a read-only reference to the metadata stored in the internal state.
    ///
    /// Note that this puts a reader lock on the underlying [`RwLock`], so
    /// shouldn't be held onto for longer than is necessary.
    pub fn metadata(&self) -> impl std::ops::Deref<Target = Metadata> + '_ + Send + Sync {
        RwLockReadGuard::map(self.state.read(), |s| &s.metadata)
    }

    pub fn run(&mut self, task_name: &str, arguments: &str) -> EngineResult<()> {
        if !arguments.is_empty() {
            log_warning!("argument passing from the command line is currently unsupported");
        }

        let arguments = vec![]; // TODO parse arguments instead

        let Some(call_id) = self
            .populate_metadata_for_call(task_name, arguments)
            .inspect_err(|err| report_error_new(&self.engine_state, &**err))
            .ok()
            .flatten()
        else {
            return Err(EngineError::EvalFailed);
        };

        let build_tree = generate_run_tree(call_id, &self.metadata());

        let mut task_iter = build_tree.flatten().into_iter().peekable();

        macro_rules! spawn_tasks {
            () => {
                // spawn as many tasks as possible
                while let Some(node) = task_iter.peek() {
                    // ensure no children are still running
                    {
                        let handles = self.handles.lock();
                        if node
                            .children
                            .iter()
                            .any(|c| handles.contains_key(&c.call_id))
                        {
                            break;
                        }
                    }

                    // advance the iterator and spawn the task
                    let node = task_iter.next().unwrap();
                    self.spawn_task(node)?;

                    // don't add any more tasks if this one is blocking
                    let metadata = self.metadata();
                    let call = metadata.get_task_call(node.call_id).unwrap();
                    let concurrent = metadata.get_task(call.task_id).unwrap().flags.concurrent;
                    if !concurrent {
                        break;
                    }
                }
            };
        }

        let runtime =
            Runtime::new().map_err(|_| EngineError::internal("failed to create runtime"))?;
        let _rt = runtime.enter();

        // run the main loop
        runtime.block_on(async move {
            // initialize first task(s)
            spawn_tasks!();

            // join tasks and continue to add more
            while let Some(result) = self.task_pool.join_next().await {
                let (task_call_id, success) = match result {
                    Ok(Ok(result)) => result,
                    Ok(Err(error)) => {
                        self.abort_all();
                        return Err(error);
                    }
                    // join error
                    Err(err) => {
                        if err.is_cancelled() {
                            continue;
                        }

                        return Err(EngineError::internal(format!("failed to join task: {err}")));
                    }
                };

                // ensure the handle is removed
                // FIXME remove handle in every brnach instead
                self.handles.lock().remove(&task_call_id);

                if !success {
                    self.abort_all();

                    let task_name = {
                        let metadata = self.metadata();
                        let task_id = metadata.get_task_call(task_call_id).unwrap().task_id;
                        metadata.get_task(task_id).unwrap().name.item.clone()
                    };

                    return Err(EngineError::TaskFailed { task_name });
                }

                spawn_tasks!();
            }

            Ok(())
        })
    }

    fn populate_metadata_for_call(
        &mut self,
        task_name: &str,
        arguments: Vec<Argument>,
    ) -> DiagResult<Option<TaskCallId>> {
        let call_id = {
            let mut state = self.state.write();

            let task_id = state.metadata.find_task_id(task_name, None)?;
            state
                .metadata
                .register_task_call(task_id, Span::unknown(), arguments, Vec::new())
                .unwrap()
        };

        if let Err(error) = self.populate_metadata_for_call_id(call_id) {
            self.report_shell_error(&error);
            return Ok(None);
        }

        Ok(Some(call_id))
    }

    fn populate_metadata_for_call_id(&mut self, call_id: TaskCallId) -> ShellResult<bool> {
        if !eval_task_decl_body(call_id, &self.engine_state, &mut self.stack)? {
            return Ok(false);
        }

        // copy out dependencies to avoid deadlock between readers/writers
        let dependencies = self
            .state
            .read()
            .metadata
            .task_call_metadata(call_id)
            .unwrap()
            .dependencies
            .clone();

        for dep_call_id in &dependencies {
            if !self.populate_metadata_for_call_id(*dep_call_id)? {
                return Ok(false);
            }
        }

        Ok(true)
    }

    fn spawn_task(&mut self, node: &RunNode) -> EngineResult<()> {
        // try to abort this task and its transitive dependencies
        self.abort_tree(node);

        // lock handles early to prevent weirdness
        let mut handles = self.handles.lock();

        // use our own engine state
        let mut engine_state = self.engine_state.clone();
        let mut stack = self.stack.clone();

        // set up ctrlc handler so we can abort tasks individually
        let ctrlc = Arc::new(AtomicBool::default());
        engine_state.ctrlc = Some(ctrlc.clone());

        let call_id = node.call_id;

        let state = self.state.clone();

        let abort_handle = self.task_pool.spawn(async move {
            let (name, call_span) = {
                let state = state.read();

                let call = state.metadata.get_task_call(call_id).unwrap();
                let call_span = call.span;
                let name = state
                    .metadata
                    .get_task(call.task_id)
                    .unwrap()
                    .name
                    .item
                    .clone();

                if !is_dirty(&call.metadata).map_err(|err| {
                    EngineError::internal(format!("failed to check dirty status: {err}"))
                })? {
                    log_info!("skipping task", &name);
                    return Ok((call_id, true));
                }

                (name, call_span)
            };

            log_info!("running task", &name);

            let result = eval_task_run_body(call_id, call_span, &engine_state, &mut stack);

            let success = match result {
                // silently ignore intentional interrupt errors
                Err(ShellError::InterruptedByUser { .. }) => return Ok((call_id, false)),
                Err(err) => {
                    // filter out quake internal errors--these will be emitted by quake itself
                    if !err.is_quake_internal() {
                        report_error_new(&engine_state, &err);
                    }

                    false
                }
                Ok(success) => success,
            };

            Ok((call_id, success))
        });

        // insert the handle, dropping the lock
        handles.insert(node.call_id, (abort_handle, ctrlc));

        Ok(())
    }

    fn abort_all(&mut self) {
        let mut handles = self.handles.lock();
        for (_, (abort, ctrlc)) in handles.drain() {
            // set the ctrlc flag, will abort the task relatively quickly
            ctrlc.store(true, Ordering::SeqCst);
            abort.abort();
        }
    }

    fn abort_tree(&mut self, root: &RunNode) {
        if let Some((abort, ctrlc)) = self.handles.lock().get(&root.call_id) {
            ctrlc.store(true, Ordering::SeqCst);
            abort.abort();
        }

        root.children.iter().for_each(|c| self.abort_tree(c));
    }
}

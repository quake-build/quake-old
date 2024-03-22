#![feature(let_chains)]
#![feature(try_trait_v2)]
#![feature(result_flattening)]
#![allow(dead_code)]

use std::collections::HashMap;
use std::fs;
use std::process::exit;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use nu_parser::parse;
use nu_protocol::ast::Argument;
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
    task_pool: JoinSet<Result<(TaskCallId, bool)>>,
    handles: Mutex<HashMap<TaskCallId, (AbortHandle, Arc<AtomicBool>)>>,
}

impl Engine {
    pub fn load(project: Project, options: EngineOptions) -> Result<Self> {
        #[cfg(windows)]
        nu_ansi_term::enable_ansi_support();

        let state = Arc::new(RwLock::new(State::new()));

        let engine_state = create_engine_state(state.clone())?;
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

        if !engine.load_script()? {
            log_fatal!("failed to load build script");
            exit(exit_codes::LOAD_FAIL);
        }

        Ok(engine)
    }

    /// Load and evaluate the project's build script.
    fn load_script(&mut self) -> Result<bool> {
        let build_script = self.project.build_script();
        let filename = build_script
            .strip_prefix(self.project.project_root())
            .unwrap_or(build_script)
            .to_string_lossy()
            .into_owned();

        let source = fs::read_to_string(build_script)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to read build script `{filename}`"))?;

        if !self.eval_source(source.as_bytes(), &filename)? {
            return Ok(false);
        }

        Ok(true)
    }

    /// Evaluate the source of a build file, returning whether or not an error
    /// occurred.
    fn eval_source(&mut self, source: &[u8], filename: &str) -> Result<bool> {
        // parse the build script
        let (block, delta) = {
            let mut working_set = StateWorkingSet::new(&self.engine_state);

            // perform a first-pass parse over the file
            let mut output = parse(&mut working_set, Some(filename), source, false);

            let mut success = true;

            // extract task metadata by reparsing
            let mut state = self.state.write();
            if let Err(errors) = parse_metadata(&mut output, &mut state.metadata, &mut working_set)
            {
                success = false;
                for error in &errors {
                    report_error(&working_set, error);
                }
            };

            // TODO print more than one error?
            if let Some(err) = working_set.parse_errors.first() {
                success = false;
                report_error(&working_set, err);
            }

            if !success {
                return Ok(false);
            }

            (output, working_set.render())
        };

        // merge updated state
        if let Err(err) = self.engine_state.merge_delta(delta) {
            report_error_new(&self.engine_state, &err);
            return Ok(false);
        }

        // evaluate the build script
        let result = eval_block(&block, &self.engine_state, &mut self.stack);
        if let Err(err) = &result {
            report_error_new(&self.engine_state, err);
        }

        Ok(result.is_ok())
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

    pub fn run(&mut self, task_name: &str, arguments: &str) -> Result<()> {
        if !arguments.is_empty() {
            log_warning!("argument passing from the command line is currently unsupported");
        }

        let arguments = vec![]; // TODO parse arguments instead
        let call_id = self.populate_metadata_for_call(task_name, arguments)?;

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

        let runtime = Runtime::new().into_diagnostic()?;
        let _rt = runtime.enter();

        // run the main loop
        runtime.block_on(async move {
            // initialize first task(s)
            spawn_tasks!();

            // join tasks and continue to add more
            while let Some(result) = self.task_pool.join_next().await {
                let (task_id, success) = result
                    .into_diagnostic()
                    .context("Failed to join task")
                    .flatten()?;

                self.handles.lock().remove(&task_id);

                if !success {
                    log_fatal!("aborting due to failed task");
                    self.abort_all();
                    exit(exit_codes::TASK_RUN_FAIL);
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
    ) -> Result<TaskCallId> {
        let call_id = {
            let mut state = self.state.write();

            let task_id = state.metadata.find_task_id(task_name, None)?;
            state
                .metadata
                .register_task_call(task_id, Span::unknown(), arguments, Vec::new())
                .unwrap()
        };

        match self.populate_metadata_for_call_id(call_id) {
            Err(err) => {
                report_error_new(&self.engine_state, &err);
                exit(exit_codes::TASK_DECL_FAIL);
            }
            Ok(false) => {
                exit(exit_codes::TASK_DECL_FAIL);
            }
            Ok(true) => Ok(call_id),
        }
    }

    fn populate_metadata_for_call_id(
        &mut self,
        call_id: TaskCallId,
    ) -> std::result::Result<bool, ShellError> {
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

    fn spawn_task(&mut self, node: &RunNode) -> Result<()> {
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

                if !is_dirty(&call.metadata)? {
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
                    report_error_new(&engine_state, &err);
                    false
                }
                Ok(success) => success,
            };

            if !success {
                log_error!("task failed", &name);
            }

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

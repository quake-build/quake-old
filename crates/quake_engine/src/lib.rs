#![feature(let_chains)]
#![feature(result_flattening)]
#![allow(dead_code)]

use std::collections::HashMap;
use std::fs;
use std::process::exit;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use nu::eval::eval_task_run_body;
use nu_parser::parse;
use nu_protocol::ast::Argument;
use nu_protocol::engine::{EngineState, Stack, StateWorkingSet};
use nu_protocol::{report_error, report_error_new, ShellError, Span, VarId};
use parking_lot::{Mutex, MutexGuard};
use parse::parse_def_tasks;
use tokio::runtime::Runtime;
use tokio::task::{AbortHandle, JoinSet};

use quake_core::exit_codes;
use quake_core::prelude::*;

use crate::eval::{eval_block, eval_task_decl_body};
use crate::metadata::{Metadata, TaskCallId};
use crate::nu::{create_engine_state, create_stack};
use crate::run_tree::{generate_run_tree, RunNode};
use crate::state::State;
use crate::utils::*;

pub(crate) mod nu;
pub(crate) use nu::{eval, parse};

mod state;
pub use state::metadata;

pub mod utils;

mod run_tree;

/// The ID of the `$quake` variable, which holds the internal state of the program.
///
/// The value of this constant is the next available variable ID after the first five that are
/// reserved by nushell. If for some reason this should change in the future, this discrepancy
/// should be noticed by an assertion following the registration of the variable into the working
/// set.
pub const QUAKE_VARIABLE_ID: VarId = 5;

/// The ID of the `$quake_scope` variable, which is set inside evaluated blocks in order to retrieve
/// scoped state from the global state.
pub const QUAKE_SCOPE_VARIABLE_ID: VarId = 6;

/// The custom nushell [`Category`](::nu_protocol::Category) assigned to quake items.
pub const QUAKE_CATEGORY: &str = "quake";

/// The custom nushell [`Category`](::nu_protocol::Category) assigned to internal quake items.
pub const QUAKE_INTERNAL_CATEGORY: &str = "quake";

#[derive(Debug, Clone)]
pub struct Options {
    pub quiet: bool,
}

pub struct Engine {
    project: Project,
    _options: Options,
    state: Arc<Mutex<State>>,
    engine_state: EngineState,
    stack: Stack,
    task_pool: JoinSet<Result<(TaskCallId, bool)>>,
    handles: Mutex<HashMap<TaskCallId, (AbortHandle, Arc<AtomicBool>)>>,
}

impl Engine {
    pub fn new(project: Project, options: Options) -> Result<Self> {
        #[cfg(windows)]
        nu_ansi_term::enable_ansi_support();

        let internal_state = Arc::new(Mutex::new(State::new()));

        let engine_state = create_engine_state(internal_state.clone())?;
        let stack = create_stack(project.project_root());

        let mut engine = Self {
            project,
            _options: options,
            state: internal_state,
            engine_state,
            stack,
            task_pool: JoinSet::new(),
            handles: Mutex::new(HashMap::new()),
        };

        if !engine.load()? {
            exit(exit_codes::LOAD_FAIL);
        }

        Ok(engine)
    }

    /// Load and evaluate the project's build script.
    fn load(&mut self) -> Result<bool> {
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

    /// Evaluate the source of a build file, returning whether or not an error occurred.
    fn eval_source(&mut self, source: &[u8], filename: &str) -> Result<bool> {
        let (block, delta) = {
            let mut working_set = StateWorkingSet::new(&self.engine_state);

            // perform a first-pass parse over the file
            let mut output = parse(&mut working_set, Some(filename), source, false);

            // re-parse `def-task` calls, populating the metadata with the corresponding task stubs
            let mut state = self.state.lock();
            parse_def_tasks(&mut output, &mut working_set, &mut state.metadata);

            if let Some(err) = working_set.parse_errors.first() {
                set_last_exit_code(&mut self.stack, 101);
                report_error(&working_set, err);
                return Ok(false);
            }

            (output, working_set.render())
        };

        // merge updated state
        if let Err(err) = self.engine_state.merge_delta(delta) {
            set_last_exit_code(&mut self.stack, 101);
            report_error_new(&self.engine_state, &err);
            return Ok(false);
        }

        // evaluate the build script again,
        let result = eval_block(&block, &self.engine_state, &mut self.stack);
        if let Err(err) = &result {
            report_error_new(&self.engine_state, err);
        }

        Ok(result.is_ok())
    }

    pub fn project(&self) -> &Project {
        &self.project
    }

    /// Get the metadata stored in the internal state.
    ///
    /// Note that this locks the internal state (which is used elsewhere frequently, including in
    /// command implementations), so shouldn't be held onto for longer than is necessary.
    pub fn metadata(&self) -> impl std::ops::Deref<Target = Metadata> + '_ {
        MutexGuard::map(self.state.lock(), |s| &mut s.metadata)
    }

    pub fn run(&mut self, task_name: &str, arguments: impl Into<Vec<Argument>>) -> Result<()> {
        let arguments = arguments.into();

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
                    let concurrent = metadata
                        .get_task_stub(call.task_id)
                        .unwrap()
                        .flags
                        .concurrent;
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
            let mut state = self.state.lock();

            let task_id = state.metadata.find_task_stub_id(task_name)?;
            state
                .metadata
                .register_task_call(task_id, arguments, Span::unknown())
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

        // again avoiding deadlock, cheap clone
        let call = self
            .state
            .lock()
            .metadata
            .get_task_call(call_id)
            .unwrap()
            .metadata
            .clone()
            .expect("no metadata defined for task, was decl body run?");

        for dep_call_id in &call.dependencies {
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
        let call = self.metadata().get_task_call(call_id).unwrap().clone();

        let name = self
            .metadata()
            .get_task_stub(call.task_id)
            .expect("invalid task ID for call")
            .name
            .item
            .clone();

        let abort_handle = self.task_pool.spawn(async move {
            let metadata = call
                .metadata
                .as_ref()
                .expect("no metadata defined for task, was decl body run?");
            if !is_dirty(metadata)? {
                print_info("skipping task", &name);

                return Ok((call_id, true));
            }

            print_info("running task", &name);

            // TODO replace this span with the calling origin
            let result = eval_task_run_body(call_id, call.span, &engine_state, &mut stack);

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
                print_error("task failed", &name);
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

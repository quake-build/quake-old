#![feature(let_chains)]

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::exit;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use metadata::TaskId;
use nu_cli::gather_parent_env_vars;
use nu_cmd_lang::create_default_context;
use nu_command::add_shell_command_context;
use nu_parser::parse;
use nu_protocol::ast::Block;
use nu_protocol::engine::{EngineState, Stack, StateWorkingSet, PWD_ENV};
use nu_protocol::{
    print_if_stream, report_error, report_error_new, PipelineData, ShellError, Span, Type, Value,
    VarId,
};
use parking_lot::{Mutex, MutexGuard};
use run_tree::RunNode;
use tokio::runtime::Runtime;
use tokio::task::{AbortHandle, JoinSet};

use quake_core::prelude::*;

use crate::metadata::{Metadata, Task};
use crate::run_tree::generate_run_tree;
use crate::state::{State, StateVariable};

pub mod metadata;

mod commands;
mod run_tree;
mod state;

mod utils;
pub use utils::*;

/// The ID of the `$quake` variable, which holds the internal state of the
/// program.
///
/// The value of this constant is the next available variable ID after the first
/// five that are reserved by nushell. If for some reason this should change in
/// the future, this discrepancy should be noticed by an assertion following the
/// registration of the variable into the working set.
pub const QUAKE_VARIABLE_ID: VarId = 5;

/// The ID of the `$quake_scope` variable, which is set inside evaluated blocks
/// in order to retrieve scoped state from the global state.
pub const QUAKE_SCOPE_VARIABLE_ID: VarId = 6;

#[derive(Debug, Clone)]
pub struct Options {
    pub quiet: bool,
}

pub struct Engine {
    project: Project,
    _options: Options,
    internal_state: Arc<Mutex<State>>,
    engine_state: EngineState,
    stack: Stack,
    task_pool: JoinSet<Result<(TaskId, bool)>>,
    handles: Mutex<HashMap<TaskId, (AbortHandle, Arc<AtomicBool>)>>,
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
            internal_state,
            engine_state,
            stack,
            task_pool: JoinSet::new(),
            handles: Mutex::new(HashMap::new()),
        };

        if !engine.load()? {
            exit(1);
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

        if !self.eval_source(source.as_bytes(), &filename) {
            return Ok(false);
        }

        Ok(true)
    }

    /// Evaluate the source of a build file, returning whether or not an error
    /// occurred.
    fn eval_source(&mut self, source: &[u8], filename: &str) -> bool {
        let (block, delta) = {
            let mut working_set = StateWorkingSet::new(&self.engine_state);

            let output = parse(&mut working_set, Some(filename), source, false);
            if let Some(err) = working_set.parse_errors.first() {
                set_last_exit_code(&mut self.stack, 1);
                report_error(&working_set, err);
                return false;
            }

            // add $quake_scope to the captures of all blocks
            for block_id in 0..working_set.num_blocks() {
                working_set
                    .get_block_mut(block_id)
                    .captures
                    .push(QUAKE_SCOPE_VARIABLE_ID);
            }

            (output, working_set.render())
        };

        if let Err(err) = self.engine_state.merge_delta(delta) {
            set_last_exit_code(&mut self.stack, 1);
            report_error_new(&self.engine_state, &err);
            return false;
        }

        let result = eval_block(&block, &self.engine_state, &mut self.stack);
        if let Err(err) = &result {
            report_error_new(&self.engine_state, err);
        }
        result.is_ok()
    }

    pub fn project(&self) -> &Project {
        &self.project
    }

    pub fn metadata(&self) -> impl std::ops::Deref<Target = Metadata> + '_ {
        MutexGuard::map(self.internal_state.lock(), |s| &mut s.metadata)
    }

    pub fn run(&mut self, task: &str) -> Result<()> {
        let build_tree = {
            let metadata = self.metadata();
            let task_id = metadata.get_global_task_id(task)?;
            generate_run_tree(task_id, &metadata)
        };

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
                            .any(|c| handles.contains_key(&c.task_id))
                        {
                            break;
                        }
                    }

                    // advance the iterator and spawn the task
                    let node = task_iter.next().unwrap();
                    self.spawn_task(node)?;

                    // don't add any more tasks if this one is blocking
                    if !self.metadata().get_task(node.task_id).unwrap().concurrent {
                        break;
                    }
                }
            };
        }

        let runtime = Runtime::new().into_diagnostic()?;
        _ = runtime.enter();

        // run the main loop
        runtime.block_on(async move {
            // initialize first task(s)
            spawn_tasks!();

            // join tasks and continue to add more
            while let Some(result) = self.task_pool.join_next().await {
                let (task_id, success) = result.unwrap()?;

                self.handles.lock().remove(&task_id);

                if !success {
                    self.abort_all();
                    exit(1);
                }

                spawn_tasks!();
            }

            Ok(())
        })
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

        let task_id = node.task_id;
        let task = self.metadata().get_task(task_id).unwrap().clone();

        let abort_handle = self.task_pool.spawn(async move {
            if !is_dirty(&task)? {
                if let Some(name) = &task.name {
                    print_info("skipping task", &name.item);
                }

                return Ok((task_id, true));
            }

            if let Some(name) = &task.name {
                print_info("running task", &name.item);
            }

            if let Some(run_block) = task.run_block {
                let block = engine_state.get_block(run_block);

                // subtasks accept an argument--apply that here
                if let Some((arg_id, value)) = &task.argument {
                    stack.add_var(*arg_id, value.clone());
                }

                let result = eval_block(block, &engine_state, &mut stack);
                let success = match result {
                    // silently ignore interrupt errors
                    Err(ShellError::InterruptedByUser { .. }) => return Ok((task_id, false)),
                    Err(err) => {
                        report_error_new(&engine_state, &err);
                        false
                    }
                    Ok(success) => success,
                };

                if !success && let Some(name) = &task.name {
                    print_error("task failed", &name.item);
                }

                Ok((task_id, success))
            } else {
                Ok((task_id, true))
            }
        });

        // insert the handle, dropping the lock
        handles.insert(node.task_id, (abort_handle, ctrlc));

        Ok(())
    }

    fn abort_all(&mut self) {
        let mut handles = self.handles.lock();
        for (_, (abort, ctrlc)) in handles.drain() {
            ctrlc.store(true, Ordering::SeqCst);
            abort.abort();
        }
    }

    fn abort_tree(&mut self, root: &RunNode) {
        if let Some((abort, ctrlc)) = self.handles.lock().get(&root.task_id) {
            ctrlc.store(true, Ordering::SeqCst);
            abort.abort();
        }

        root.children.iter().for_each(|c| self.abort_tree(c));
    }
}

fn create_engine_state(state: Arc<Mutex<State>>) -> Result<EngineState> {
    let mut engine_state = add_shell_command_context(create_default_context());

    // TODO merge with PWD logic below
    gather_parent_env_vars(&mut engine_state, Path::new("."));

    let delta = {
        use crate::commands::*;

        let mut working_set = StateWorkingSet::new(&engine_state);

        macro_rules! bind_global_variable {
            ($name:expr, $id:expr, $type:expr) => {
                let var_id = working_set.add_variable($name.into(), Span::unknown(), $type, false);
                assert_eq!(
                    var_id, $id,
                    concat!("ID variable of `", $name, "` did not match predicted value")
                );
            };
        }

        bind_global_variable!("$quake", QUAKE_VARIABLE_ID, Type::Any);
        bind_global_variable!("$quake_scope", QUAKE_SCOPE_VARIABLE_ID, Type::Int);

        working_set.set_variable_const_val(
            QUAKE_VARIABLE_ID,
            Value::custom_value(Box::new(StateVariable(state)), Span::unknown()),
        );

        macro_rules! bind_command {
            ($($command:expr),* $(,)?) => {
                $(working_set.add_decl(Box::new($command));)*
            };
        }

        bind_command! {
            DefTask,
            Subtask,
            Depends,
            Sources,
            Produces
        };

        working_set.render()
    };

    engine_state
        .merge_delta(delta)
        .expect("Failed to register custom engine state");

    Ok(engine_state)
}

fn create_stack(cwd: impl AsRef<Path>) -> Stack {
    let mut stack = Stack::new();

    stack.add_env_var(
        PWD_ENV.to_owned(),
        Value::String {
            val: cwd.as_ref().to_string_lossy().to_string(),
            internal_span: Span::unknown(),
        },
    );

    stack.add_var(QUAKE_SCOPE_VARIABLE_ID, Value::int(-1, Span::unknown()));

    stack
}

fn eval_block(
    block: &Block,
    engine_state: &EngineState,
    stack: &mut Stack,
) -> std::result::Result<bool, ShellError> {
    if block.is_empty() {
        return Ok(true);
    }

    let result = nu_engine::eval_block_with_early_return(
        engine_state,
        stack,
        block,
        PipelineData::Empty,
        false,
        false,
    );

    match result {
        Ok(pipeline_data) => {
            let result = if let PipelineData::ExternalStream {
                stdout: stream,
                stderr: stderr_stream,
                exit_code,
                ..
            } = pipeline_data
            {
                print_if_stream(stream, stderr_stream, false, exit_code)
            } else {
                pipeline_data.drain_with_exit_code()
            };

            match result {
                Ok(exit_code) => {
                    set_last_exit_code(stack, exit_code);
                    if exit_code != 0 {
                        return Ok(false);
                    }
                }
                Err(err) => {
                    return Err(err);
                }
            }

            // reset vt processing, aka ansi because illbehaved externals can break it
            #[cfg(windows)]
            {
                let _ = nu_utils::enable_vt_processing();
            }
        }
        Err(err) => {
            set_last_exit_code(stack, 1);
            return Err(err);
        }
    }

    Ok(true)
}

fn is_dirty(task: &Task) -> Result<bool> {
    // if either is undefined, assume dirty
    if task.sources.is_empty() || task.artifacts.is_empty() {
        return Ok(true);
    }

    // TODO glob from PWD?

    let (sources, artifacts) = (expand_globs(&task.sources)?, expand_globs(&task.artifacts)?);

    Ok(latest_timestamp(&sources)? > latest_timestamp(&artifacts)?)
}

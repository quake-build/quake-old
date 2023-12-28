use std::fs;
use std::path::Path;
use std::sync::Arc;

use miette::bail;
use nu_ansi_term::{Color, Style};
use nu_cli::gather_parent_env_vars;
use nu_cmd_lang::create_default_context;
use nu_command::add_shell_command_context;
use nu_engine::eval_block;
use nu_parser::parse;
use nu_protocol::ast::Block;
use nu_protocol::engine::{EngineState, Stack, StateWorkingSet, PWD_ENV};
use nu_protocol::{
    print_if_stream, report_error, report_error_new, BlockId, PipelineData, Span, Spanned, Type,
    Value, VarId,
};
use parking_lot::{Mutex, MutexGuard};

use quake_core::prelude::*;

use crate::metadata::{BuildMetadata, Dependency, Task};
use crate::state::{State, StateVariable};

mod commands;
mod metadata;
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

#[derive(Clone)]
pub struct Engine {
    project: Project,
    options: Options,
    internal_state: Arc<Mutex<State>>,
    engine_state: EngineState,
    stack: Stack,
    is_loaded: bool,
}

impl Engine {
    pub fn new(project: Project, options: Options) -> Result<Self> {
        #[cfg(windows)]
        nu_ansi_term::enable_ansi_support();

        let build_state = Arc::new(Mutex::new(State::new()));

        let engine_state = create_engine_state(build_state.clone())?;
        let stack = create_stack(project.project_root());

        let mut engine = Self {
            project,
            options,
            internal_state: build_state,
            engine_state,
            stack,
            is_loaded: false,
        };

        if !engine.load()? {
            bail!("Failed to read build script");
        }

        Ok(engine)
    }

    pub fn project(&self) -> &Project {
        &self.project
    }

    pub fn metadata(&self) -> impl std::ops::Deref<Target = BuildMetadata> + '_ {
        MutexGuard::map(self.internal_state.lock(), |s| &mut s.metadata)
    }

    pub fn run(&mut self, task: &str) -> Result<bool> {
        // determine a build plan (i.e. the order in which to evaluate dependencies)
        let metadata = self.internal_state.lock().metadata.clone();
        let build_plan = generate_build_plan(task, &metadata)?;

        // run all tasks in the proper order
        for run_task in &build_plan {
            if let RunTask::Task(task) = run_task {
                // perform a dirty check, only if both sources and artifacts are
                // defined
                if !is_dirty(task)? {
                    self.print_action(format!("skipping {}", &task.name.item));
                    continue;
                }
            }

            let run_block = match &run_task {
                RunTask::Task(task) => {
                    // print the run message if this task has any associated run
                    // bodies (itself or any subtasks)
                    if task.run_block.is_some()
                        || build_plan.iter().any(|t| {
                            matches!(t, RunTask::Subtask { parent, .. }
                                     if parent.item == task.name.item)
                        })
                    {
                        self.print_action(format!("starting {}", &task.name.item));
                    }
                    task.run_block
                }
                RunTask::Subtask {
                    name,
                    block_id,
                    argument,
                    ..
                } => {
                    self.print_action(format!("starting {}", &name.item));

                    // bind the closure argument to the saved data, if it exists
                    if let Some(var) = argument {
                        self.stack.add_var(var.0, var.1.clone());
                    }
                    Some(*block_id)
                }
            };

            if let Some(run_block) = run_block {
                let block = self.engine_state.get_block(run_block).clone();
                if !self.eval_block(&block) {
                    let task_name = match run_task {
                        RunTask::Task(task) => &task.name.item,
                        RunTask::Subtask { name, .. } => &name.item,
                    };
                    self.print_error(format!("failed {task_name}"));
                    return Ok(false);
                }
            }
        }

        self.print_action(format!("finished {task}"));

        Ok(true)
    }

    fn load(&mut self) -> Result<bool> {
        assert!(!self.is_loaded, "build script should only be loaded once");

        let build_script = self.project.build_script();
        let filename = build_script
            .strip_prefix(self.project.project_root())
            .unwrap_or(build_script)
            .to_string_lossy()
            .to_string();

        let source = fs::read_to_string(build_script)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to read build script `{filename}`"))?;

        if !self.eval_source(source.as_bytes(), &filename) {
            return Ok(false);
        }

        // validate the parsed results
        self.internal_state.lock().metadata.validate()?;

        // set so that we don't load again
        self.is_loaded = true;

        Ok(true)
    }

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

        self.eval_block(&block)
    }

    fn eval_block(&mut self, block: &Block) -> bool {
        if block.is_empty() {
            return true;
        }

        let result = eval_block(
            &self.engine_state,
            &mut self.stack,
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
                    Err(err) => {
                        report_error_new(&self.engine_state, &err);
                        return false;
                    }
                    Ok(exit_code) => {
                        set_last_exit_code(&mut self.stack, exit_code);
                        if exit_code != 0 {
                            return false;
                        }
                    }
                }

                // reset vt processing, aka ansi because illbehaved externals can break it
                #[cfg(windows)]
                {
                    let _ = nu_utils::enable_vt_processing();
                }
            }
            Err(err) => {
                set_last_exit_code(&mut self.stack, 1);
                report_error_new(&self.engine_state, &err);
                return false;
            }
        }

        true
    }

    fn print_action(&self, message: impl AsRef<str>) {
        self.print_message(message.as_ref(), Color::White);
    }

    fn print_error(&self, message: impl AsRef<str>) {
        self.print_message(message.as_ref(), Color::LightRed);
    }

    #[inline]
    fn print_message(&self, message: &str, color: Color) {
        if !self.options.quiet {
            eprintln!(
                "{} {message}",
                Style::new().fg(color).bold().paint("> quake:"),
            );
        }
    }
}

#[derive(Debug, Clone)]
pub struct Options {
    pub quiet: bool,
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

#[derive(PartialEq)]
enum RunTask<'a> {
    Task(&'a Task),
    Subtask {
        parent: Spanned<String>,
        name: Spanned<String>,
        block_id: BlockId,
        argument: Option<(VarId, Value)>,
    },
}

fn generate_build_plan<'a>(task: &str, metadata: &'a BuildMetadata) -> Result<Vec<RunTask<'a>>> {
    // NOTE metadata is assumed to have been validated

    fn add_deps<'a>(task: &'a Task, run_tasks: &mut Vec<RunTask<'a>>, metadata: &'a BuildMetadata) {
        for dep in &task.dependencies {
            match dep {
                Dependency::Task(dep) => {
                    let task = &metadata.tasks[&dep.item];
                    if !run_tasks.contains(&RunTask::Task(task)) {
                        add_deps(task, run_tasks, metadata);
                    }
                }
                Dependency::Subtask {
                    parent,
                    name,
                    block_id,
                    argument,
                } => run_tasks.push(RunTask::Subtask {
                    parent: parent.clone(),
                    name: name.clone(),
                    block_id: *block_id,
                    argument: argument.clone(),
                }),
            }
        }
        run_tasks.push(RunTask::Task(task));
    }

    let root = metadata
        .tasks
        .get(task)
        .ok_or_else(|| errors::TaskNotFound {
            task: task.to_owned(),
        })?;
    let mut run_tasks = vec![];
    add_deps(root, &mut run_tasks, metadata);

    Ok(run_tasks)
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

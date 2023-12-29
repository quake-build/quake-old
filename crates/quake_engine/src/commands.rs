use nu_engine::{eval_block, CallExt};
use nu_protocol::ast::Call;
use nu_protocol::engine::{Block, Closure, Command, EngineState, Stack};
use nu_protocol::{
    Category, PipelineData, ShellError, Signature, Spanned, SyntaxShape, Type, Value,
};

use quake_core::prelude::IntoShellError;

use crate::metadata::{Task, TaskKind};
use crate::state::{Scope, State};

const QUAKE_CATEGORY: &str = "quake";

#[derive(Clone)]
pub struct DefTask;

impl Command for DefTask {
    fn name(&self) -> &str {
        "def-task"
    }

    fn usage(&self) -> &str {
        "Define a quake task"
    }

    fn signature(&self) -> Signature {
        Signature::build("def-task")
            .input_output_types(vec![(Type::Nothing, Type::String)])
            .required("name", SyntaxShape::String, "task name")
            .switch(
                "concurrent",
                "allow this task to be run concurrently with others",
                Some('c'),
            )
            .optional("decl_body", SyntaxShape::Block, "declarational body")
            .required("run_body", SyntaxShape::Block, "run body")
            .category(Category::Custom(QUAKE_CATEGORY.to_owned()))
    }

    fn run(
        &self,
        engine_state: &EngineState,
        stack: &mut Stack,
        call: &Call,
        input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        let name: Spanned<String> = call.req(engine_state, stack, 0)?;

        let block_0: Block = call.req(engine_state, stack, 1)?;
        let (decl_block, run_block) = match call.opt(engine_state, stack, 2)? {
            Some(block_1) => (Some(block_0), block_1),
            None => (None, block_0),
        };

        let state = State::from_engine_state(engine_state);

        let task = Task::new(
            name.clone(),
            TaskKind::Global,
            Some(run_block.block_id),
            call.has_flag("concurrent"),
        );
        if let Some(block) = &decl_block {
            state
                .lock()
                .push_scope(Scope::new(task), stack, call.span());

            let block = engine_state.get_block(block.block_id);
            eval_block(engine_state, stack, block, input, false, false)?;

            let mut state = state.lock();
            let task = state
                .pop_scope(stack, call.span())
                .map_err(IntoShellError::into_shell_error)?
                .task;
            state.metadata.register_task(task);
        } else {
            state.lock().metadata.register_task(task);
        }

        Ok(PipelineData::Value(
            Value::String {
                val: name.item,
                internal_span: name.span,
            },
            None,
        ))
    }
}

#[derive(Clone)]
pub struct Subtask;

impl Command for Subtask {
    fn name(&self) -> &str {
        "subtask"
    }

    fn usage(&self) -> &str {
        "Define and depend upon an anonymous subtask"
    }

    fn signature(&self) -> Signature {
        Signature::build("subtask")
            .input_output_types(vec![(Type::Any, Type::String)])
            .required("name", SyntaxShape::String, "subtask name")
            .switch(
                "concurrent",
                "allow this task to be run concurrently with others",
                Some('c'),
            )
            .required(
                "run_body",
                SyntaxShape::Closure(Some(vec![SyntaxShape::Any])),
                "run body",
            )
            .category(Category::Custom(QUAKE_CATEGORY.to_owned()))
    }

    fn run(
        &self,
        engine_state: &EngineState,
        stack: &mut Stack,
        call: &Call,
        input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        let span = call.span();

        let (name, closure) = (
            call.req::<Spanned<String>>(engine_state, stack, 0)?,
            call.req::<Closure>(engine_state, stack, 1)?,
        );

        let block = engine_state.get_block(closure.block_id);

        let state = State::from_engine_state(engine_state);
        {
            let mut state = state.lock();

            let mut subtask = Task::new(
                name.clone(),
                TaskKind::Subtask,
                Some(closure.block_id),
                call.has_flag("concurrent"),
            );

            if let Some(argument) = block
                .signature
                .required_positional
                .first()
                .and_then(|arg| arg.var_id)
                .map(|v| (v, input.into_value(span)))
            {
                subtask.argument = Some(argument);
            }

            let subtask_id = state.metadata.register_task(subtask);

            let task = &mut state
                .get_scope_mut(stack, span)
                .map_err(IntoShellError::into_shell_error)?
                .task;
            task.dependencies.push(subtask_id);
        }

        Ok(PipelineData::Value(
            Value::String {
                val: name.item,
                internal_span: name.span,
            },
            None,
        ))
    }
}

#[derive(Clone)]
pub struct Depends;

impl Command for Depends {
    fn name(&self) -> &str {
        "depends"
    }

    fn signature(&self) -> Signature {
        Signature::build("depends")
            .input_output_types(vec![(Type::Nothing, Type::Nothing)])
            .required("dep_id", SyntaxShape::String, "dependency ID")
            .category(Category::Custom(QUAKE_CATEGORY.to_owned()))
    }

    fn usage(&self) -> &str {
        "Depend on another quake task"
    }

    fn run(
        &self,
        engine_state: &EngineState,
        stack: &mut Stack,
        call: &Call,
        _input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        let span = call.span();

        let dep: Spanned<String> = call.req(engine_state, stack, 0)?;

        let state = State::from_engine_state(engine_state);
        {
            let mut state = state.lock();
            let dep_id = state
                .metadata
                .get_global_task_id(&dep.item)
                .map_err(IntoShellError::into_shell_error)?;
            state
                .get_scope_mut(stack, span)
                .map_err(IntoShellError::into_shell_error)?
                .task
                .dependencies
                .push(dep_id);
        }

        Ok(PipelineData::empty())
    }
}

#[derive(Clone)]
pub struct Sources;

impl Command for Sources {
    fn name(&self) -> &str {
        "sources"
    }

    fn signature(&self) -> Signature {
        Signature::build("sources")
            .input_output_types(vec![(Type::Nothing, Type::Nothing)])
            .required(
                "files",
                SyntaxShape::List(Box::new(SyntaxShape::String)),
                "files to be sourced",
            )
            .category(Category::Custom(QUAKE_CATEGORY.to_owned()))
    }

    fn usage(&self) -> &str {
        "Declare files to be sourced by a task"
    }

    fn run(
        &self,
        engine_state: &EngineState,
        stack: &mut Stack,
        call: &Call,
        _input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        let span = call.span();

        let values: Vec<Spanned<String>> = call.req(engine_state, stack, 0)?;

        State::from_engine_state(engine_state)
            .lock()
            .get_scope_mut(stack, span)
            .map_err(IntoShellError::into_shell_error)?
            .task
            .sources
            .extend(values);

        Ok(PipelineData::empty())
    }
}

#[derive(Clone)]
pub struct Produces;

impl Command for Produces {
    fn name(&self) -> &str {
        "produces"
    }

    fn signature(&self) -> Signature {
        Signature::build("produces")
            .input_output_types(vec![(Type::Nothing, Type::Nothing)])
            .required(
                "files",
                SyntaxShape::List(Box::new(SyntaxShape::String)),
                "files to be produced",
            )
            .category(Category::Custom(QUAKE_CATEGORY.to_owned()))
    }

    fn usage(&self) -> &str {
        "Declare files to be produced by a task"
    }

    fn run(
        &self,
        engine_state: &EngineState,
        stack: &mut Stack,
        call: &Call,
        _input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        let span = call.span();

        let values: Vec<Spanned<String>> = call.req(engine_state, stack, 0)?;

        State::from_engine_state(engine_state)
            .lock()
            .get_scope_mut(stack, span)
            .map_err(IntoShellError::into_shell_error)?
            .task
            .artifacts
            .extend(values);

        Ok(PipelineData::empty())
    }
}

use nu_engine::CallExt;
use nu_protocol::ast::Call;
use nu_protocol::engine::{Closure, Command, EngineState, Stack};
use nu_protocol::{
    Category, PipelineData, ShellError, Signature, Spanned, SyntaxShape, Type, Value,
};

use super::QUAKE_CATEGORY;

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
            .input_output_types(vec![(Type::Nothing, Type::Nothing)])
            .required("name", SyntaxShape::String, "task name")
            .switch(
                "concurrent",
                "allow this task to be run concurrently with others",
                Some('c'),
            )
            // HACK this should be a SyntaxShape::Signature, but declaring it as such here causes
            // the second, optional block to get passed over, so we use a close enough syntactical
            // approximation (a list<any>) and re-parse the span manually as a signature after the
            // initial parsing pass
            .required(
                "params",
                SyntaxShape::List(Box::new(SyntaxShape::Any)),
                "parameters",
            )
            .optional("decl_body", SyntaxShape::Closure(None), "declaration body")
            .required("run_body", SyntaxShape::Closure(None), "run body")
            .category(Category::Custom(QUAKE_CATEGORY.to_owned()))
    }

    fn run(
        &self,
        _engine_state: &EngineState,
        _stack: &mut Stack,
        _call: &Call,
        _input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        // (parser internal)
        Ok(PipelineData::Empty)
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
        _input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        let _span = call.span();

        let (name, closure) = (
            call.req::<Spanned<String>>(engine_state, stack, 0)?,
            call.req::<Closure>(engine_state, stack, 1)?,
        );

        let _block = engine_state.get_block(closure.block_id);

        // let state = State::from_engine_state(engine_state);
        // {
        //     let mut state = state.lock();

        //     let mut subtask = TaskMetadata::new(
        //         name.clone(),
        //         TaskKind::Subtask,
        //         Some(closure.block_id),
        //         call.has_flag("concurrent"),
        //     );

        //     if let Some(argument) = block
        //         .signature
        //         .required_positional
        //         .first()
        //         .and_then(|arg| arg.var_id)
        //         .map(|v| (v, input.into_value(span)))
        //     {
        //         subtask.argument = Some(argument);
        //     }

        //     let subtask_id = state.metadata.register_task(subtask);

        //     let task = &mut state
        //         .get_scope_mut(stack, span)
        //         .map_err(IntoShellError::into_shell_error)?
        //         .task;
        //     task.dependencies.push(subtask_id);
        // }

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
            .rest(
                "args",
                SyntaxShape::Any,
                "arguments to pass into the dependency",
            )
            .allows_unknown_args()
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
        let _span = call.span();

        let _dep: Spanned<String> = call.req(engine_state, stack, 0)?;

        // let state = State::from_engine_state(engine_state);
        // {
        //     let mut state = state.lock();
        //     let dep_id = state
        //         .metadata
        //         .get_public_task_id(&dep.item)
        //         .map_err(IntoShellError::into_shell_error)?;
        //     state
        //         .get_scope_mut(stack, span)
        //         .map_err(IntoShellError::into_shell_error)?
        //         .task
        //         .dependencies
        //         .push(dep_id);
        // }

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
        let _span = call.span();

        let _values: Vec<String> = call.req(engine_state, stack, 0)?;

        // State::from_engine_state(engine_state)
        //     .lock()
        //     .get_scope_mut(stack, span)
        //     .map_err(IntoShellError::into_shell_error)?
        //     .task
        //     .sources
        //     .extend(values);

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
        let _span = call.span();

        let _values: Vec<String> = call.req(engine_state, stack, 0)?;

        // State::from_engine_state(engine_state)
        //     .lock()
        //     .get_scope_mut(stack, span)
        //     .map_err(IntoShellError::into_shell_error)?
        //     .task
        //     .artifacts
        //     .extend(values);

        Ok(PipelineData::empty())
    }
}

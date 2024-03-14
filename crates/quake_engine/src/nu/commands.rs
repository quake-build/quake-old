use std::sync::Arc;

use nu_engine::CallExt;
use nu_protocol::ast::Call;
use nu_protocol::engine::{Closure, Command, EngineState, Stack};
use nu_protocol::{
    Category, PipelineData, ShellError, Signature, Span, Spanned, SyntaxShape, Type, Value,
};
use quake_core::errors::IntoShellResult;
use quake_core::metadata::{Task, TaskCallId, TaskFlags};

use crate::state::State;

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
            .required("params", SyntaxShape::Signature, "parameters")
            .required("decl_body", SyntaxShape::Closure(None), "declaration body")
            .required("run_body", SyntaxShape::Closure(None), "run body")
            .creates_scope()
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
            .required("run_body", SyntaxShape::Closure(None), "run body")
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

        let mut state = State::from_engine_state_mut(engine_state);
        state.check_in_scope(stack, span)?;

        let (name, closure) = (
            call.req::<Spanned<String>>(engine_state, stack, 0)?,
            call.req::<Closure>(engine_state, stack, 1)?,
        );
        let flags = TaskFlags {
            concurrent: call.has_flag(engine_state, stack, "concurrent")?,
        };

        let block = engine_state.get_block(closure.block_id);

        let mut constants = Vec::with_capacity(1);

        if let Some(arg) = block.signature.required_positional.first() {
            let expected_ty = arg.shape.to_type();
            if let PipelineData::Value(value, _) = input {
                let value_type = value.get_type();

                if !value_type.is_subtype(&expected_ty) {
                    return Err(ShellError::OnlySupportsThisInputType {
                        exp_input_type: expected_ty.to_string(),
                        wrong_type: value_type.to_string(),
                        dst_span: span,
                        src_span: value.span(),
                    });
                }

                constants.push((arg.var_id.unwrap(), value));
            } else {
                let arg_span = engine_state.get_var(arg.var_id.unwrap()).declaration_span;
                return Err(ShellError::UnsupportedInput {
                    msg: format!("subtask expected input of type {expected_ty}, but got nothing"),
                    input: "argument defined here".to_owned(),
                    msg_span: Span::new(span.start, span.start),
                    input_span: arg_span,
                });
            }
        }

        let task_id = state
            .metadata
            .register_task(
                name.item.clone(),
                Arc::new(Task {
                    name: name.clone(),
                    flags,
                    depends_decl_id: None,
                    decl_body: None,
                    run_body: Some(closure.block_id),
                }),
            )
            .into_shell_result()?;

        let call_id = state
            .metadata
            .register_task_call(task_id, span, Vec::new(), constants)
            .unwrap();
        state
            .scope_metadata_mut(stack, span)?
            .dependencies
            .push(call_id);

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
        // (parse internal, replaced with `DependsTask`)

        // emit an error if used in an invalid location
        //
        // TODO do this during the parse phase
        State::from_engine_state(engine_state).check_in_scope(stack, call.head)?;

        Ok(PipelineData::empty())
    }
}

#[derive(Clone)]
pub struct DependsTask {
    pub task_id: TaskCallId,
    pub signature: Box<Signature>,
}

impl Command for DependsTask {
    fn name(&self) -> &str {
        &self.signature.name
    }

    fn signature(&self) -> Signature {
        *self.signature.clone()
    }

    fn usage(&self) -> &str {
        &self.signature.usage
    }

    fn run(
        &self,
        engine_state: &EngineState,
        stack: &mut Stack,
        call: &Call,
        _input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        let mut state = State::from_engine_state_mut(engine_state);
        state.check_in_scope(stack, call.head)?;

        // register the call_id and add it as a dependency
        let call_id = state
            .metadata
            .register_task_call(
                self.task_id,
                call.span(),
                call.arguments.clone(),
                Vec::new(),
            )
            .unwrap();
        state
            .scope_metadata_mut(stack, call.head)?
            .dependencies
            .push(call_id);

        Ok(PipelineData::Empty)
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
        let values: Vec<String> = call.req(engine_state, stack, 0)?;

        State::from_engine_state_mut(engine_state)
            .scope_metadata_mut(stack, span)?
            .sources
            .extend(values.iter().map(Into::into));

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
        let values: Vec<String> = call.req(engine_state, stack, 0)?;

        State::from_engine_state_mut(engine_state)
            .scope_metadata_mut(stack, span)?
            .artifacts
            .extend(values.iter().map(Into::into));

        Ok(PipelineData::empty())
    }
}

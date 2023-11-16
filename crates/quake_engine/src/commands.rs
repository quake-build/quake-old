use nu_engine::{eval_block, CallExt};
use nu_protocol::ast::Call;
use nu_protocol::engine::{Block, Closure, Command, EngineState, Stack};
use nu_protocol::{Category, PipelineData, ShellError, Signature, Spanned, SyntaxShape, Type};

use crate::metadata::{Dependency, Task};
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
            .input_output_types(vec![(Type::Nothing, Type::Nothing)])
            .required("def_name", SyntaxShape::String, "task name")
            .optional("decl_body", SyntaxShape::Block, "declarational body")
            .required("run_body", SyntaxShape::Block, "run body")
            .switch(
                "invisible",
                "make this an \"invisible\" task with no run body",
                Some('i'),
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
        let name: Spanned<String> = call.req(engine_state, stack, 0)?;

        let invisible = call.has_flag("invisible");

        let block_0: Block = call.req(engine_state, stack, 1)?;
        let (decl_block, run_block) = if !invisible {
            match call.opt(engine_state, stack, 2)? {
                Some(block_1) => (Some(block_0), Some(block_1)),
                None => (None, Some(block_0)),
            }
        } else {
            (Some(block_0), None)
        };

        let state = State::from_engine_state(engine_state).unwrap();

        let task = Task::new(name, run_block.map(|b| b.block_id));
        if let Some(block) = &decl_block {
            state
                .lock()
                .unwrap()
                .push_scope(Scope::TaskDecl(task), stack, call.span());

            let block = engine_state.get_block(block.block_id);
            eval_block(engine_state, stack, block, input, false, false)?;

            let mut state = state.lock().unwrap();
            let Scope::TaskDecl(task) = state.pop_scope(stack, call.span()).unwrap(); // TODO handle scope mismatch
            state.metadata.tasks.insert(task.name.item.clone(), task);
        } else {
            state
                .lock()
                .unwrap()
                .metadata
                .tasks
                .insert(task.name.item.clone(), task);
        }

        Ok(PipelineData::empty())
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
            .input_output_types(vec![(Type::Any, Type::Nothing)])
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

        let closure: Closure = call.req(engine_state, stack, 0)?;
        let block = engine_state.get_block(closure.block_id);

        let argument = block
            .signature
            .required_positional
            .first()
            .and_then(|arg| arg.var_id)
            .map(|v| (v, input.into_value(span)));

        let state = State::from_engine_state(engine_state).unwrap();
        {
            let mut state = state.lock().unwrap();
            let Scope::TaskDecl(task) = state.get_scope_mut(stack, span).unwrap(); // TODO handle error
            task.dependencies.push(Dependency::Anonymous {
                parent: task.name.clone(),
                block_id: closure.block_id,
                argument,
            });
        }

        Ok(PipelineData::empty())
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

        let state = State::from_engine_state(engine_state).unwrap();
        {
            let mut state = state.lock().unwrap();
            let Scope::TaskDecl(task) = state.get_scope_mut(stack, span).unwrap(); // TODO handle error
            task.dependencies.push(Dependency::Named(dep));
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

        let state = State::from_engine_state(engine_state).unwrap();
        {
            let mut state = state.lock().unwrap();
            let Scope::TaskDecl(task) = state.get_scope_mut(stack, span).unwrap(); // TODO handle error
            task.sources.extend(values);
        }

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

        let state = State::from_engine_state(engine_state).unwrap();
        {
            let mut state = state.lock().unwrap();
            let Scope::TaskDecl(task) = state.get_scope_mut(stack, span).unwrap(); // TODO handle error
            task.artifacts.extend(values);
        }

        Ok(PipelineData::empty())
    }
}

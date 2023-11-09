use nu_engine::{eval_block, CallExt};
use nu_protocol::engine::{Block, Command};
use nu_protocol::{Category, PipelineData, ShellError, Signature, Spanned, SyntaxShape, Type};

use crate::metadata::Task;
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

    fn signature(&self) -> nu_protocol::Signature {
        Signature::build("def-task")
            .input_output_types(vec![(Type::Nothing, Type::Nothing)])
            .required("def_name", SyntaxShape::String, "task name")
            .optional("decl_body", SyntaxShape::Block, "declarational body")
            .required("run_body", SyntaxShape::Block, "run body")
            .category(Category::Custom(QUAKE_CATEGORY.to_owned()))
    }

    fn run(
        &self,
        engine_state: &nu_protocol::engine::EngineState,
        stack: &mut nu_protocol::engine::Stack,
        call: &nu_protocol::ast::Call,
        input: nu_protocol::PipelineData,
    ) -> Result<nu_protocol::PipelineData, nu_protocol::ShellError> {
        let name: Spanned<String> = call.req(engine_state, stack, 0)?;

        let block_0: Block = call.req(engine_state, stack, 1)?;
        let (decl_block, run_block) = match call.opt(engine_state, stack, 2)? {
            Some(block_1) => (Some(block_0), block_1),
            None => (None, block_0),
        };

        let state = State::from_stack(stack, call.span()).unwrap();

        let task = Task::new(name, run_block.block_id);
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
        engine_state: &nu_protocol::engine::EngineState,
        stack: &mut nu_protocol::engine::Stack,
        call: &nu_protocol::ast::Call,
        _input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        let span = call.span();

        let dep: Spanned<String> = call.req(engine_state, stack, 0)?;

        let state = State::from_stack(stack, span).unwrap();
        {
            let mut state = state.lock().unwrap();
            let Scope::TaskDecl(task) = state.get_scope_mut(stack, span).unwrap(); // TODO handle error
            task.depends.push(dep);
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
        engine_state: &nu_protocol::engine::EngineState,
        stack: &mut nu_protocol::engine::Stack,
        call: &nu_protocol::ast::Call,
        _input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        let span = call.span();

        let values: Vec<Spanned<String>> = call.req(engine_state, stack, 0)?;

        let state = State::from_stack(stack, span).unwrap();
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
        engine_state: &nu_protocol::engine::EngineState,
        stack: &mut nu_protocol::engine::Stack,
        call: &nu_protocol::ast::Call,
        _input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        let span = call.span();

        let values: Vec<Spanned<String>> = call.req(engine_state, stack, 0)?;

        let state = State::from_stack(stack, span).unwrap();
        {
            let mut state = state.lock().unwrap();
            let Scope::TaskDecl(task) = state.get_scope_mut(stack, span).unwrap(); // TODO handle error
            task.produces.extend(values);
        }

        Ok(PipelineData::empty())
    }
}

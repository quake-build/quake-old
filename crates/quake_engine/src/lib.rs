use std::fs;

use nu_protocol::engine::{EngineState, Stack};
use nu_protocol::PipelineData;

use quake_core::prelude::*;

mod helpers;
pub use helpers::*;

#[derive(Clone)]
pub struct Engine {
    pub project: Project,
    pub engine_state: EngineState,
    pub stack: Stack,
}

impl Engine {
    pub fn new(project: Project) -> Result<Self> {
        let engine_state = create_engine_state();
        let stack = create_stack(project.project_root());

        Ok(Self {
            project,
            engine_state,
            stack,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        let source = fs::read_to_string(self.project.build_script())
            .into_diagnostic()
            .wrap_err("Failed to read build script")?;
        eval_source(
            &mut self.engine_state,
            &mut self.stack,
            source.as_bytes(),
            "build.quake",
            PipelineData::Empty,
            true,
        );
        Ok(())
    }
}

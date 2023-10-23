use std::fs;

use nu_engine::eval_block;
use nu_parser::parse;
use nu_protocol::engine::{EngineState, Stack, StateWorkingSet};
use nu_protocol::{print_if_stream, report_error, report_error_new, PipelineData};

use quake_core::prelude::*;

mod helpers;
pub use helpers::*;

#[derive(Clone)]
pub struct Engine {
    project: Project,
    engine_state: EngineState,
    stack: Stack,
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

    pub fn project(&self) -> &Project {
        &self.project
    }

    pub fn run(&mut self) -> Result<bool> {
        let source = fs::read_to_string(self.project.build_script())
            .into_diagnostic()
            .wrap_err("Failed to read build script")?;
        Ok(self.eval_source(source.as_bytes(), "build.quake"))
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

            (output, working_set.render())
        };

        if let Err(err) = self.engine_state.merge_delta(delta) {
            set_last_exit_code(&mut self.stack, 1);
            report_error_new(&self.engine_state, &err);
            return false;
        }

        let result = eval_block(
            &self.engine_state,
            &mut self.stack,
            &block,
            PipelineData::Empty,
            false,
            false,
        );

        match result {
            Ok(pipeline_data) => {
                let result;
                if let PipelineData::ExternalStream {
                    stdout: stream,
                    stderr: stderr_stream,
                    exit_code,
                    ..
                } = pipeline_data
                {
                    result = print_if_stream(stream, stderr_stream, false, exit_code);
                } else {
                    result = pipeline_data.print(&self.engine_state, &mut self.stack, true, false);
                }

                match result {
                    Err(err) => {
                        report_error_new(&self.engine_state, &err);
                        return false;
                    }
                    Ok(exit_code) => {
                        set_last_exit_code(&mut self.stack, exit_code);
                    }
                }

                // reset vt processing, aka ansi because illbehaved externals can break it
                #[cfg(windows)]
                {
                    let _ = enable_vt_processing();
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
}

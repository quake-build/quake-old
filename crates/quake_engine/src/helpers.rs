// Taken directly from https://github.com/jntrnr/nu_app (MIT licensed) to just
// get something working.

use std::path::{Path, PathBuf};

use nu_cmd_lang::create_default_context;
use nu_command::add_shell_command_context;

use nu_protocol::engine::{EngineState, Stack};
use nu_protocol::{Span, Value};

pub fn set_last_exit_code(stack: &mut Stack, exit_code: i64) {
    stack.add_env_var(
        "LAST_EXIT_CODE".to_string(),
        Value::int(exit_code, Span::unknown()),
    );
}

pub fn get_init_cwd() -> Option<PathBuf> {
    std::env::current_dir()
        .ok()
        .or_else(|| std::env::var("PWD").ok().map(Into::into))
}

pub fn create_engine_state() -> EngineState {
    // For now, just use the commands that are included in nushell by default.
    // Eventually, we will be more selective.
    let engine_state = create_default_context();
    add_shell_command_context(engine_state)
}

pub fn create_stack(cwd: impl AsRef<Path>) -> Stack {
    // stack
    let mut stack = Stack::new();

    stack.add_env_var(
        "PWD".into(),
        Value::String {
            val: cwd.as_ref().to_string_lossy().to_string(),
            internal_span: Span::unknown(),
        },
    );

    stack
}

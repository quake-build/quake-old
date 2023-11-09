use std::path::PathBuf;

use nu_protocol::engine::{Stack, PWD_ENV};
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
        .or_else(|| std::env::var(PWD_ENV).ok().map(Into::into))
}

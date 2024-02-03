use nu_protocol::engine::Stack;
use nu_protocol::{Span, Value};

pub fn set_last_exit_code(stack: &mut Stack, exit_code: i64) {
    stack.add_env_var(
        "LAST_EXIT_CODE".to_owned(),
        Value::int(exit_code, Span::unknown()),
    );
}

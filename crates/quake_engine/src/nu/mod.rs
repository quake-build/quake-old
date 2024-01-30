use std::path::Path;
use std::sync::Arc;

use nu_cli::gather_parent_env_vars;
use nu_cmd_lang::create_default_context;
use nu_command::add_shell_command_context;
use nu_protocol::engine::{EngineState, Stack, StateWorkingSet, PWD_ENV};
use nu_protocol::{Span, Type, Value};
use parking_lot::Mutex;

use crate::state::State;
use crate::{QUAKE_SCOPE_VARIABLE_ID, QUAKE_VARIABLE_ID};

pub mod commands;
pub mod eval;
pub mod parse;
pub mod types;

pub fn create_engine_state(state: Arc<Mutex<State>>) -> crate::Result<EngineState> {
    let mut engine_state = add_shell_command_context(create_default_context());

    // TODO merge with PWD logic below
    gather_parent_env_vars(&mut engine_state, Path::new("."));

    let delta = {
        use commands::*;

        let mut working_set = StateWorkingSet::new(&engine_state);

        macro_rules! bind_global_variable {
            ($name:expr, $id:expr, $type:expr) => {
                let var_id = working_set.add_variable($name.into(), Span::unknown(), $type, false);
                assert_eq!(
                    var_id, $id,
                    concat!("ID variable of `", $name, "` did not match predicted value")
                );
            };
        }

        bind_global_variable!("$quake", QUAKE_VARIABLE_ID, Type::Any);
        bind_global_variable!("$quake_scope", QUAKE_SCOPE_VARIABLE_ID, Type::Int);

        working_set.set_variable_const_val(
            QUAKE_VARIABLE_ID,
            Value::custom_value(Box::new(types::State(state)), Span::unknown()),
        );

        macro_rules! bind_command {
            ($($command:expr),* $(,)?) => {
                $(working_set.add_decl(Box::new($command));)*
            };
        }

        bind_command! {
            DefTask,
            Subtask,
            Depends,
            Sources,
            Produces
        };

        working_set.render()
    };

    engine_state
        .merge_delta(delta)
        .expect("Failed to register custom engine state");

    Ok(engine_state)
}

pub fn create_stack(cwd: impl AsRef<Path>) -> Stack {
    let mut stack = Stack::new();

    stack.add_env_var(
        PWD_ENV.to_owned(),
        Value::String {
            val: cwd.as_ref().to_string_lossy().to_string(),
            internal_span: Span::unknown(),
        },
    );

    stack.add_var(QUAKE_SCOPE_VARIABLE_ID, Value::int(-1, Span::unknown()));

    stack
}

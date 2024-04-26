use std::path::Path;
use std::sync::Arc;

use nu_cli::gather_parent_env_vars;
use nu_cmd_lang::create_default_context;
use nu_command::add_shell_command_context;
use nu_protocol::engine::{EngineState, Stack, StateWorkingSet, PWD_ENV};
use nu_protocol::{Span, Type, Value, VarId};
use parking_lot::RwLock;

use crate::state::State;

pub mod commands;
pub mod eval;
pub mod parse;
pub mod types;
pub mod utils;

/// The ID of the `$quake` variable, which holds the internal state of the
/// program.
///
/// The value of this constant is the next available variable ID after the first
/// five that are reserved by nushell. If for some reason this should change in
/// the future, this discrepancy should be noticed by an assertion following the
/// registration of the variable into the working set.
pub const QUAKE_VARIABLE_ID: VarId = 5;

/// The ID of the `$quake_scope` variable, which is set inside evaluated blocks
/// in order to retrieve scoped state from the global state.
pub const QUAKE_SCOPE_VARIABLE_ID: VarId = 6;

/// The name for the custom nushell [`Category`](::nu_protocol::Category)
/// assigned to quake commands.
pub const QUAKE_CATEGORY: &str = "quake";

pub fn create_engine_state(state: Arc<RwLock<State>>) -> EngineState {
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
            nu_cli::NuHighlight,
            nu_cli::Print,
        };

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

    engine_state
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

use std::sync::Arc;

use nu_protocol::ast::{Argument, Block};
use nu_protocol::engine::{EngineState, Stack};
use nu_protocol::{print_if_stream, PipelineData, ShellError, Span, Value};

use quake_core::prelude::IntoShellError;

use crate::metadata::{TaskCallId, TaskCallMetadata};
use crate::state::{Scope, State};
use crate::utils::set_last_exit_code;

pub fn eval_block(
    block: &Block,
    engine_state: &EngineState,
    stack: &mut Stack,
) -> Result<bool, ShellError> {
    if block.is_empty() {
        return Ok(true);
    }

    let result = nu_engine::eval_block_with_early_return(
        engine_state,
        stack,
        block,
        PipelineData::Empty,
        false,
        false,
    );

    match result {
        Ok(pipeline_data) => {
            let result = if let PipelineData::ExternalStream {
                stdout: stream,
                stderr: stderr_stream,
                exit_code,
                ..
            } = pipeline_data
            {
                print_if_stream(stream, stderr_stream, false, exit_code)
            } else {
                pipeline_data.drain_with_exit_code()
            };

            match result {
                Ok(exit_code) => {
                    set_last_exit_code(stack, exit_code);
                    if exit_code != 0 {
                        return Ok(false);
                    }
                }
                Err(err) => {
                    return Err(err);
                }
            }

            // reset vt processing, aka ansi because illbehaved externals can break it
            #[cfg(windows)]
            {
                let _ = nu_utils::enable_vt_processing();
            }
        }
        Err(err) => {
            set_last_exit_code(stack, 1);
            return Err(err);
        }
    }

    Ok(true)
}

pub fn eval_task_decl_body(
    call_id: TaskCallId,
    engine_state: &EngineState,
    stack: &mut Stack,
) -> Result<bool, ShellError> {
    let state = State::from_engine_state(engine_state);

    // convert task stub into task metadata
    let (call, meta, decl_body) = {
        let mut state = state.lock();

        let call = state.metadata.get_task_call(call_id).unwrap().clone();

        let meta = Arc::new(TaskCallMetadata::default());

        let stub = state.metadata.get_task_stub(call.task_id).unwrap();
        let Some(decl_body) = stub.decl_body else {
            // no decl body: early return with no additional metadata
            state.metadata.insert_task_call_metadata(call_id, meta);
            return Ok(true);
        };

        (call, meta, decl_body)
    };

    // push task scope (will error if nested inside another task body)
    state
        .lock()
        .push_scope(Scope::new(meta), stack, call.span)
        .map_err(IntoShellError::into_shell_error)?;

    // evaluate declaration body
    let block = engine_state.get_block(decl_body);
    let success = eval_block_with_args(block, &call.arguments, call.span, engine_state, stack)?;

    // pop task scope and register into metadata
    let mut state = state.lock();
    let task = state
        .pop_scope(stack, call.span)
        .map_err(IntoShellError::into_shell_error)?
        .task;

    state.metadata.insert_task_call_metadata(call_id, task);

    Ok(success)
}

pub fn eval_task_run_body(
    call_id: TaskCallId,
    span: Span,
    engine_state: &EngineState,
    stack: &mut Stack,
) -> Result<bool, ShellError> {
    let state = State::from_engine_state(engine_state);

    let (block_id, call) = {
        let state = state.lock();

        let call = state.metadata.get_task_call(call_id).unwrap().clone(); // cheap clone
        let block_id = state.metadata.get_task_stub(call.task_id).unwrap().run_body;

        if block_id.is_none() {
            return Ok(true);
        }

        (block_id.unwrap(), call)
    };

    let block = engine_state.get_block(block_id);
    let result = eval_block_with_args(block, &call.arguments, span, engine_state, stack)?;

    Ok(result)
}

/// Similar to [`eval_call`](nu_engine::eval_call), but with manual blocks and arguments.
fn eval_block_with_args(
    block: &Block,
    arguments: &[Argument],
    span: Span,
    engine_state: &EngineState,
    stack: &mut Stack,
) -> Result<bool, ShellError> {
    let signature = &block.signature;

    let mut positional_arg_vals = Vec::with_capacity(arguments.len());
    let mut named_arg_vals = Vec::with_capacity(arguments.len());
    let mut rest_arg_val = None;

    for arg in arguments {
        match arg {
            Argument::Positional(a) => positional_arg_vals.push(a),
            Argument::Named(a) => named_arg_vals.push(a),
            Argument::Spread(rest) => rest_arg_val = Some(rest),
            Argument::Unknown(_) => unimplemented!("Argument::Unknown in task call"),
        }
    }

    let mut callee_stack = stack.gather_captures(engine_state, &block.captures);

    for (param_idx, param) in signature
        .required_positional
        .iter()
        .chain(signature.optional_positional.iter())
        .enumerate()
    {
        let value = if let Some(expr) = positional_arg_vals.get(param_idx) {
            nu_engine::eval_expression(engine_state, stack, expr)?
        } else if let Some(value) = &param.default_value {
            value.clone()
        } else {
            Value::nothing(span)
        };

        callee_stack.add_var(param.var_id.unwrap(), value);
    }

    if let (Some(rest_arg), Some(rest_val)) = (&signature.rest_positional, &rest_arg_val) {
        callee_stack.add_var(
            rest_arg.var_id.unwrap(),
            nu_engine::eval_expression(engine_state, stack, rest_val)?,
        );
    }

    for named in &signature.named {
        let var_id = named.var_id.unwrap();

        let value = if let Some(expr) = named_arg_vals
            .iter()
            .find(|(long, short, _)| {
                named.long == long.item
                    || named.short == short.as_ref().and_then(|s| s.item.chars().next())
            })
            .map(|(_, _, expr)| expr)
        {
            if let Some(expr) = expr {
                nu_engine::eval_expression(engine_state, stack, expr)?
            } else if let Some(value) = &named.default_value {
                value.clone()
            } else {
                Value::bool(true, span)
            }
        } else if named.arg.is_none() {
            Value::bool(false, span)
        } else if let Some(value) = &named.default_value {
            value.clone()
        } else {
            Value::nothing(span)
        };

        callee_stack.add_var(var_id, value);
    }

    eval_block(block, engine_state, &mut callee_stack)
}

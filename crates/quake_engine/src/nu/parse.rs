use std::mem;
use std::sync::Arc;

use nu_parser::{discover_captures_in_expr, parse_internal_call};
use nu_protocol::ast::{Argument, Block, Call, Expr, Expression, PipelineElement};
use nu_protocol::engine::StateWorkingSet;
use nu_protocol::{span, Category, Span, Spanned, Type};

use quake_core::metadata::{Metadata, Task, TaskFlags};
use quake_core::prelude::*;

use crate::nu::commands::DependsTask;

use super::{QUAKE_CATEGORY, QUAKE_SCOPE_VARIABLE_ID};

pub fn parse_metadata(
    block: &mut Block,
    metadata: &mut Metadata,
    working_set: &mut StateWorkingSet<'_>,
) -> std::result::Result<(), Vec<ShellError>> {
    let mut lazy_errors = Vec::new();

    // register tasks in the metadata, creating new depends decls for each task
    for call in calls_in_block(working_set, "def-task", block, false) {
        match parse_def_task(call, working_set, metadata) {
            Ok(LazyResult::Failure { errors }) => lazy_errors.extend(errors),
            Err(err) => lazy_errors.push(err),
            _ => {}
        }
    }

    if lazy_errors.is_empty() {
        Ok(())
    } else {
        Err(lazy_errors)
    }
}

// TODO impl try? with fast failure and lazy failure

#[derive(Debug, Clone)]
enum LazyResult {
    Success,
    Failure { errors: Vec<ShellError> },
}

impl LazyResult {
    pub const fn success() -> Self {
        Self::Success
    }

    pub const fn failure(errors: Vec<ShellError>) -> Self {
        Self::Failure { errors }
    }

    pub const fn is_success(&self) -> bool {
        matches!(self, Self::Success)
    }
}

/// Reparse and register a `def-task` block as a task.
///
/// Returns whether or not the call was successfully
fn parse_def_task(
    call: &mut Call,
    working_set: &mut StateWorkingSet<'_>,
    metadata: &mut Metadata,
) -> ShellResult<LazyResult> {
    // errors to emit at the end if it is not a critical failure
    let mut late_errors = vec![];

    // extract name--must be const eval
    let name: Spanned<String> = call.req_const(working_set, 0)?;

    // try to extract flags
    let flags = TaskFlags {
        concurrent: call.has_flag_const(working_set, "concurrent")?,
    };

    let is_declarative = call.has_flag_const(working_set, "decl")?;

    // extract and update signature in place
    let Some(Expression {
        expr: Expr::Signature(signature),
        ..
    }) = call.positional_nth_mut(1)
    else {
        return Ok(LazyResult::failure(late_errors));
    };
    signature.name.clone_from(&name.item);
    signature.category = Category::Custom(QUAKE_CATEGORY.to_owned());

    let signature = signature.clone();

    let mut closures = call.arguments.iter().filter_map(|a| {
        if let Some(Expression {
            expr: Expr::Closure(block_id),
            ..
        }) = a.expression()
        {
            Some(*block_id)
        } else {
            None
        }
    });

    // extract block IDs
    let (Some(first_block), second_block) = (closures.next(), closures.next()) else {
        return Ok(LazyResult::failure(late_errors));
    };

    // update signature for the first block
    working_set
        .get_block_mut(first_block)
        .signature
        .clone_from(&signature);

    // determine which blocks correspond to which bodies
    let (run_body, decl_body) = match second_block {
        Some(second_block) => {
            if is_declarative {
                // too many blocks: add error and continue
                late_errors.push(
                    errors::DeclTaskHasExtraBody {
                        span: working_set.get_block(second_block).span.unwrap(),
                    }
                    .into_shell_error(),
                );

                (Some(first_block), None)
            } else {
                // update the signature for the second block
                working_set
                    .get_block_mut(second_block)
                    .signature
                    .clone_from(&signature);
                (Some(second_block), Some(first_block))
            }
        }
        None => {
            if is_declarative {
                (None, Some(first_block))
            } else {
                (Some(first_block), None)
            }
        }
    };
    // insert placeholder to be updated later with a `DependsTask` if successful
    let depends_decl_name = format!("depends {name}", name = &name.item);
    let depends_decl_id = {
        let task_id = metadata.next_task_id();

        // modify the signature to mimick that of the `Depends` command
        let mut signature = signature;
        signature.name.clone_from(&depends_decl_name);

        let decl_id = working_set.add_decl(Box::new(DependsTask { task_id, signature }));
        working_set
            .last_overlay_mut()
            .visibility
            .hide_decl_id(&decl_id);
        decl_id
    };

    if let Some(decl_body) = decl_body {
        let mut block = mem::take(working_set.get_block_mut(decl_body));

        // add the task call scope ID variable to the block's captures
        block.captures.push(QUAKE_SCOPE_VARIABLE_ID);

        // transform `Depends` calls to `DependsTask`
        for call in calls_in_block(working_set, "depends", &mut block, true) {
            transform_depends(call, working_set, metadata)?;
        }

        *working_set.get_block_mut(decl_body) = block;
    }

    // remove errors indicating a missing argument when only one block is provided
    if run_body.is_some() != decl_body.is_some() {
        let call_span = call.span();
        working_set.parse_errors.retain(|e| {
            !matches!(e, ParseError::MissingPositional(name, span, _)
                                  if name == "second_body" && call_span.contains_span(*span))
        });
    }

    // note: errors when task has already been defined
    if let Err(err) = metadata.register_task(
        name.item.clone(),
        Arc::new(Task {
            name,
            flags,
            depends_decl_id: Some(depends_decl_id),
            decl_body,
            run_body,
        }),
    ) {
        working_set.error(err.into_parse_error());

        // clean up to prevent future collisions
        working_set
            .last_overlay_mut()
            .decls
            .remove(depends_decl_name.as_bytes());
    }

    if late_errors.is_empty() {
        Ok(LazyResult::success())
    } else {
        Ok(LazyResult::failure(late_errors))
    }
}

fn transform_depends(
    call: &mut Box<Call>,
    working_set: &mut StateWorkingSet<'_>,
    metadata: &mut Metadata,
) -> ShellResult<()> {
    // update the decl id to the corresponding `DependsTask` command
    // extract dep name--must be const eval
    let dep_id: Spanned<String> = call.req_const(working_set, 0)?;

    // remove the name span
    let name_span = call.arguments.remove(0).span();

    // find the decl ID to the corresponding `DependsTask` command
    let depends_decl_id = metadata
        .find_task(&dep_id.item, Some(dep_id.span))
        .into_shell_result()?
        .depends_decl_id
        .ok_or(errors::TaskCannotDepend {
            name: dep_id.item,
            span: name_span,
        })
        .into_diagnostic()
        .into_shell_result()?;

    *call = {
        working_set.enter_scope();

        // figure out which captures (if any) are used inside the call
        //
        // ignore errors, as this will have already been called during the initial parse
        let (mut seen, mut seen_blocks, mut output) = Default::default();
        drop(discover_captures_in_expr(
            working_set,
            &Expression {
                expr: Expr::Call(call.clone()),
                span: call.span(),
                ty: Type::Nothing,
                custom_completion: None,
            },
            &mut seen,
            &mut seen_blocks,
            &mut output,
        ));

        // add all found captures to the current overlay
        for (var_id, var_span) in output {
            // TODO find a more foolproof way to determine var name
            let var_name = working_set.get_span_contents(var_span).to_owned();
            working_set
                .last_overlay_mut()
                .insert_variable(var_name, var_id);
        }

        // reparse the call with captures now in scope
        let arg_spans = call
            .arguments
            .iter()
            .map(Argument::span)
            .collect::<Vec<_>>();
        let call = parse_internal_call(
            working_set,
            span(&[call.head, name_span]),
            &arg_spans,
            depends_decl_id,
        )
        .call;

        working_set.exit_scope();

        call
    };

    Ok(())
}

/// Get all valid calls of a particular declaration inside a given block.
///
/// ## Panics
///
/// If there is no corresponding decl in the working set for the given command.
fn calls_in_block<'a>(
    working_set: &StateWorkingSet<'_>,
    decl_name: &str,
    block: &'a mut Block,
    skip_errors: bool,
) -> impl Iterator<Item = &'a mut Box<Call>> {
    let decl_id = working_set
        .find_decl(decl_name.as_bytes())
        .unwrap_or_else(|| panic_bug!("command {decl_name} not defined"));

    let error_spans: Vec<Span> = if skip_errors {
        let block_span = block.span.unwrap();
        working_set
            .parse_errors
            .iter()
            .map(ParseError::span)
            .filter(|s| block_span.contains_span(*s))
            .collect()
    } else {
        Vec::new()
    };

    // FIXME: this only catches top-level calls. need to refactor to recurse
    // through the entire AST
    block
        .pipelines
        .iter_mut()
        .flat_map(|p| p.elements.iter_mut())
        .filter_map(move |pe| {
            if let PipelineElement::Expression(
                _,
                Expression {
                    expr: Expr::Call(call),
                    span: call_span,
                    ..
                },
            ) = pe
                && call.decl_id == decl_id
                && !(skip_errors && error_spans.iter().any(|s| call_span.contains_span(*s)))
            {
                Some(call)
            } else {
                None
            }
        })
}

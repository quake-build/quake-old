#![deny(clippy::wildcard_enum_match_arm)]

use std::collections::HashSet;
use std::sync::Arc;

use nu_parser::{discover_captures_in_expr, parse_internal_call, parse_value};
use nu_protocol::ast::{
    Argument, Block, Call, Expr, Expression, ExternalArgument, MatchPattern, Pattern, RecordItem,
};
use nu_protocol::engine::{Closure, StateWorkingSet};
use nu_protocol::{span, Category, DeclId, Span, Spanned, SyntaxShape, Type};

use quake_core::metadata::{Task, TaskFlags};
use quake_core::prelude::*;

use crate::nu::commands::DependsTask;
use crate::state::State;

use super::{QUAKE_CATEGORY, QUAKE_SCOPE_VARIABLE_ID};

pub fn parse_metadata(block: &mut Block, working_set: &mut StateWorkingSet<'_>, state: &mut State) {
    // register tasks in the metadata, creating new depends decls for each task
    modify_calls(working_set, b"def-task", block, |working_set, call| {
        state.capture_errors(|state| parse_def_task(call, working_set, state));
    });
}

/// Reparse and register a `def-task` block as a task.
///
/// Returns whether or not the call was successfully
fn parse_def_task(
    call: &mut Box<Call>,
    working_set: &mut StateWorkingSet<'_>,
    state: &mut State,
) -> DiagResult<()> {
    if call.has_flag_const(working_set, "help")? {
        return Ok(());
    }

    // extract name--must be const eval
    let name: Spanned<String> = call.req_const(working_set, 0)?;

    // try to extract flags--must be const eval
    let flags = TaskFlags {
        concurrent: call.has_flag_const(working_set, "concurrent")?,
    };

    // extract and update signature in place
    let Some(Expression {
        expr: Expr::Signature(signature),
        ..
    }) = call.positional_nth_mut(1)
    else {
        return Ok(());
    };
    signature.name.clone_from(&name.item);
    signature.category = Category::Custom(QUAKE_CATEGORY.to_owned());

    let signature = signature.clone();

    // extract closures by keyword
    let (mut decl_body, mut run_body) = (None, None);
    for expr in call.arguments.iter_mut().flat_map(|a| a.expression_mut()) {
        match_expr!(Expr::Keyword(kw_name, _, kw_expr), expr, else { continue; });
        let block_id = match &kw_expr.expr {
            Expr::Closure(block_id) => *block_id,
            Expr::Garbage => {
                *kw_expr = Box::new(parse_value(
                    working_set,
                    kw_expr.span,
                    &SyntaxShape::Closure(None),
                ));
                match_expr!(Expr::Closure(block_id), **kw_expr, else {
                    // indiciative of extra positional
                    break;
                });
                block_id
            }
            _ => unreachable!(),
        };

        match kw_name.as_slice() {
            b"where" => decl_body = Some(block_id),
            b"do" => run_body = Some(block_id),
            _ => unreachable!(),
        };
    }

    if decl_body.is_none() && run_body.is_none() {
        state.error(errors::TaskMissingBlocks { span: call.span() });
        return Ok(());
    }

    // update signatures
    for block_id in [decl_body, run_body].iter_mut().filter_map(Option::as_mut) {
        working_set
            .get_block_mut(*block_id)
            .signature
            .clone_from(&signature);
    }

    // insert placeholder to be updated later with a `DependsTask` if successful
    let depends_decl_name = format!("depends {name}", name = &name.item);
    let depends_decl_id = {
        let task_id = state.metadata.next_task_id();

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
        // add the task call scope ID variable to the block's captures
        let mut block = working_set.get_block_mut(decl_body).clone();
        block.captures.push(QUAKE_SCOPE_VARIABLE_ID);

        // transform `Depends` calls to `DependsTask`
        modify_calls(working_set, b"depends", &mut block, |working_set, call| {
            state.capture_errors(|state| transform_depends(call, working_set, state));
        });

        *working_set.get_block_mut(decl_body) = block;
    }

    // remove errors indicating a missing argument when only one block is provided
    // if run_body.is_some() != decl_body.is_some() {
    let call_span = call.span();
    working_set.parse_errors.retain(|e| match e {
        ParseError::ExpectedKeyword(kw, span) | ParseError::KeywordMissingArgument(_, kw, span)
            if call_span.contains_span(*span) && (kw == "where" || kw == "do") =>
        {
            false
        }
        ParseError::Expected(_, span) if call_span.contains_span(*span) => false,
        ParseError::MissingPositional(arg, _, _)
            if arg == "decl_body" || arg.starts_with("run_body") =>
        {
            false
        }
        _ => true,
    });

    // note: errors when task has already been defined
    let name_span = name.span;
    if let Err(error) = state.metadata.register_task(
        name.item.clone(),
        Arc::new(Task {
            name,
            flags,
            depends_decl_id: Some(depends_decl_id),
            decl_body,
            run_body,
        }),
        name_span,
    ) {
        state.error(error);

        // clean up to prevent future collisions
        working_set
            .last_overlay_mut()
            .decls
            .remove(depends_decl_name.as_bytes());
    }

    Ok(())
}

fn transform_depends(
    call: &mut Box<Call>,
    working_set: &mut StateWorkingSet<'_>,
    state: &mut State,
) -> DiagResult<()> {
    // update the decl id to the corresponding `DependsTask` command
    // extract dep name--must be const eval
    let Ok(dep_id) = call.req_const::<Spanned<String>>(working_set, 0) else {
        return Ok(());
    };

    // find the decl ID to the corresponding `DependsTask` command
    let depends_decl_id = state
        .metadata
        .find_task(&dep_id.item, Some(dep_id.span))?
        .depends_decl_id
        .ok_or(errors::TaskNotFound {
            name: dep_id.item,
            span: Some(dep_id.span),
        })?;

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
            .skip(1)
            .map(Argument::span)
            .collect::<Vec<_>>();
        let call = parse_internal_call(
            working_set,
            span(&[call.head, dep_id.span]),
            &arg_spans,
            depends_decl_id,
        )
        .call;

        working_set.exit_scope();

        call
    };

    Ok(())
}

fn modify_calls(
    working_set: &mut StateWorkingSet<'_>,
    decl_name: &[u8],
    block: &mut Block,
    mut func: impl FnMut(&mut StateWorkingSet<'_>, &mut Box<Call>),
) {
    let decl_id = working_set.find_decl(decl_name).expect("invalid decl name");
    modify_calls_in_block(working_set, decl_id, block, &mut func);
}

fn modify_calls_in_block(
    working_set: &mut StateWorkingSet<'_>,
    decl_id: DeclId,
    block: &mut Block,
    func: &mut dyn FnMut(&mut StateWorkingSet<'_>, &mut Box<Call>),
) {
    for expr in block
        .pipelines
        .iter_mut()
        .flat_map(|p| &mut p.elements)
        .map(|pe| &mut pe.expr)
    {
        modify_calls_in_expr(working_set, decl_id, expr, func);
    }
}

fn modify_calls_in_expr(
    working_set: &mut StateWorkingSet<'_>,
    decl_id: DeclId,
    expr: &mut Expression,
    func: &mut dyn FnMut(&mut StateWorkingSet<'_>, &mut Box<Call>),
) {
    match &mut expr.expr {
        Expr::Call(call) => {
            if call.decl_id == decl_id {
                func(working_set, call);
            }

            for arg in &mut call.arguments {
                if let Some(expr) = arg.expression_mut() {
                    modify_calls_in_expr(working_set, decl_id, expr, func);
                }
            }
        }
        Expr::Range(a, b, c, _) => {
            for expr in [a, b, c].into_iter().flatten() {
                modify_calls_in_expr(working_set, decl_id, expr, func);
            }
        }
        Expr::ExternalCall(head, args) => {
            modify_calls_in_expr(working_set, decl_id, head, func);

            for arg in args {
                let (ExternalArgument::Regular(expr) | ExternalArgument::Spread(expr)) = arg;
                modify_calls_in_expr(working_set, decl_id, expr, func);
            }
        }
        Expr::RowCondition(block_id)
        | Expr::Subexpression(block_id)
        | Expr::Block(block_id)
        | Expr::Closure(block_id) => {
            let mut block = working_set.get_block_mut(*block_id).clone();
            modify_calls_in_block(working_set, decl_id, &mut block, func);
            *working_set.get_block_mut(*block_id) = block;
        }
        Expr::UnaryNot(expr)
        | Expr::Keyword(_, _, expr)
        | Expr::ValueWithUnit(expr, _)
        | Expr::Spread(expr) => modify_calls_in_expr(working_set, decl_id, expr, func),
        Expr::BinaryOp(a, b, c) => {
            for expr in [a, b, c].into_iter() {
                modify_calls_in_expr(working_set, decl_id, expr, func);
            }
        }
        Expr::MatchBlock(match_arms) => {
            for (pat, expr) in match_arms {
                modify_calls_in_pattern(working_set, decl_id, pat, func);
                modify_calls_in_expr(working_set, decl_id, expr, func);
            }
        }
        Expr::List(exprs) | Expr::StringInterpolation(exprs) => {
            for expr in exprs {
                modify_calls_in_expr(working_set, decl_id, expr, func);
            }
        }
        Expr::Table(headers, cells) => {
            for expr in headers.iter_mut().chain(cells.iter_mut().flatten()) {
                modify_calls_in_expr(working_set, decl_id, expr, func);
            }
        }
        Expr::Record(records) => {
            for item in records {
                match item {
                    RecordItem::Pair(a, b) => {
                        modify_calls_in_expr(working_set, decl_id, a, func);
                        modify_calls_in_expr(working_set, decl_id, b, func);
                    }
                    RecordItem::Spread(_, expr) => {
                        modify_calls_in_expr(working_set, decl_id, expr, func)
                    }
                }
            }
        }
        Expr::FullCellPath(path) => {
            modify_calls_in_expr(working_set, decl_id, &mut path.head, func)
        }
        Expr::Bool(_)
        | Expr::Int(_)
        | Expr::Float(_)
        | Expr::Binary(_)
        | Expr::Var(_)
        | Expr::VarDecl(_)
        | Expr::Operator(_)
        | Expr::DateTime(_)
        | Expr::Filepath(_, _)
        | Expr::Directory(_, _)
        | Expr::GlobPattern(_, _)
        | Expr::String(_)
        | Expr::CellPath(_)
        | Expr::ImportPattern(_)
        | Expr::Overlay(_)
        | Expr::Signature(_)
        | Expr::Nothing
        | Expr::Garbage => {}
    }
}

fn modify_calls_in_pattern(
    working_set: &mut StateWorkingSet<'_>,
    decl_id: DeclId,
    pat: &mut MatchPattern,
    func: &mut dyn FnMut(&mut StateWorkingSet<'_>, &mut Box<Call>),
) {
    if let Some(expr) = &mut pat.guard {
        modify_calls_in_expr(working_set, decl_id, expr, func);
    }

    match &mut pat.pattern {
        Pattern::Record(records) => {
            for (_, pat) in records {
                modify_calls_in_pattern(working_set, decl_id, pat, func);
            }
        }
        Pattern::List(pats) | Pattern::Or(pats) => {
            for pat in pats {
                modify_calls_in_pattern(working_set, decl_id, pat, func);
            }
        }
        Pattern::Value(expr) => modify_calls_in_expr(working_set, decl_id, expr, func),
        Pattern::Variable(_)
        | Pattern::Rest(_)
        | Pattern::IgnoreRest
        | Pattern::IgnoreValue
        | Pattern::Garbage => {}
    }
}

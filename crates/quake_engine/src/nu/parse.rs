use nu_parser::{parse_full_cell_path, parse_signature};
use nu_protocol::ast::{
    Block, Expr, Expression, ExternalArgument, Pipeline, PipelineElement, RecordItem,
};
use nu_protocol::engine::{Command, StateWorkingSet};
use nu_protocol::{BlockId, Category, DeclId, Spanned};

use quake_core::prelude::*;

use crate::metadata::{Metadata, TaskFlags, TaskStub};

use super::{commands, QUAKE_CATEGORY, QUAKE_SCOPE_VARIABLE_ID};

macro_rules! match_expr {
    ($expr:pat, $arg:expr) => {
        match_expr!($expr, _, $arg)
    };
    ($expr:pat, $span:pat, $arg:expr) => {
        match_expr!($expr, $span, $arg, else {
            if cfg!(debug_assertions) {
                panic!(concat!(
                    "unexpected syntax while parsing quake syntax at ",
                    file!(),
                    ":",
                    line!(),
                    " (this is a bug)"
                ))
            } else {
                panic!("unexpected syntax while parsing quake syntax (this is a bug)")
            }
        })
    };
    ($expr:pat, $arg:expr, else $else:block) => {
        match_expr!($expr, _, $arg, else $else)
    };
    ($expr:pat, $span:pat, $arg:expr, else $else:block) => {
        let Expression { expr: $expr, span: $span, .. } = $arg else $else;
    };
}

/// Parse a given block for `def-task` invocations to transform them (and their block arguments)
/// into their expected internal representation, and return the metadata extracted from this
/// operation in the form of a collection of [`TaskStub`]s.
///
/// ## Details
///
/// `def-task` commands are [defined with a syntax](super::commands::DefTask::signature) that, in
/// order, takes:
/// - The name of the task (a string)
/// - Various flags (not positional args, so may appear anywhere)
/// - The task's signature (initially parsed as a list of strings due to a nushell parsing quirk)
/// - One or two blocks (either a run body, or a declaration body followed by a run body)
///
/// For each of these that we encounter, we apply the following transformations:
/// - Parse any flags into [`TaskFlags`]
/// - Parse the signature correctly now, and apply to to each block arg
/// - Reparse any garbage variable references with the signature now applied
pub fn parse_def_tasks(
    block: &mut Block,
    working_set: &mut StateWorkingSet<'_>,
    metadata: &mut Metadata,
) {
    let def_task_decl_id = get_cmd_decl_id(&commands::DefTask, working_set);

    for expr in calls_in_pipelines(&mut block.pipelines, def_task_decl_id) {
        parse_def_task(expr, working_set, metadata);
    }
}

fn parse_def_task(
    call_expr: &mut Expression,
    working_set: &mut StateWorkingSet<'_>,
    metadata: &mut Metadata,
) {
    // extract an owned Call from the expression, replacing the original with a stub
    match_expr!(Expr::Call(call), call_expr);

    let span = call_expr.span;

    let flags = TaskFlags {
        concurrent: call.has_flag("concurrent"),
    };

    // iterate only over positional args (which excludes any flags)
    let mut arguments = call.positional_iter_mut();

    match_expr!(Expr::String(name), name_span, arguments.next().unwrap());

    // re-parse naive list span into signature
    let sig_arg = arguments.next().unwrap();
    sig_arg.expr = Expr::List(Vec::new());
    match_expr!(
        Expr::Signature(mut signature),
        parse_signature(working_set, sig_arg.span)
    );

    // update signature details with additional information
    signature.name = name.to_owned();
    signature.category = Category::Custom(QUAKE_CATEGORY.to_owned());

    // extract block(s), and apply the above signature to them
    let mut blocks = Vec::with_capacity(2);
    for block_arg in arguments.by_ref().take(2) {
        // try and fetch another block
        match_expr!(Expr::Block(block_id), block_arg, else { break; });

        blocks.push(*block_id);

        let block = working_set.get_block_mut(*block_id);

        // add `$quake_scope` to the block's captures
        block.captures.push(QUAKE_SCOPE_VARIABLE_ID);

        // update the signature
        block.signature = signature.clone();

        // purge all errors inside the span to get rid of any bogus errors
        working_set
            .parse_errors
            .retain(|err| !span.contains_span(err.span()));

        // TODO optimization: only run when there were variable not found errors
        // reparse garbage cell paths
        reparse_garbage_paths_in_block(*block_id, working_set);
    }

    debug_assert!(
        arguments.next().is_none(),
        "extraneous args in def-task call"
    );

    // determine block IDs for metadata
    let (run_body, decl_body) = match blocks[..] {
        [a] => (Some(a), None),
        [a, b] => (Some(b), Some(a)),
        _ => unreachable!("bad def-task syntax"),
    };

    // errors when task has already been defined
    if let Err(err) = metadata.add_task_stub(
        name.clone(),
        TaskStub {
            name: Spanned {
                item: name.clone(),
                span: *name_span,
            },
            flags,
            signature,
            span,
            decl_body,
            run_body,
        },
    ) {
        working_set.error(err.into_parse_error());
    }
}

fn reparse_garbage_paths_in_block(block_id: BlockId, working_set: &mut StateWorkingSet<'_>) {
    let mut pipelines = working_set.get_block_mut(block_id).pipelines.clone(); // *sad clone noise*
    for pipeline in &mut pipelines {
        for element in &mut pipeline.elements {
            reparse_garbage_paths_in_expr(element.expression_mut(), working_set);
        }
    }
    working_set.get_block_mut(block_id).pipelines = pipelines;
}

fn reparse_garbage_paths_in_expr(expr: &mut Expression, working_set: &mut StateWorkingSet<'_>) {
    match &mut expr.expr {
        // garbage path (likely due to unbound variable)
        Expr::FullCellPath(path) if path.head.expr == Expr::Garbage => {
            *expr = parse_full_cell_path(working_set, None, expr.span);
        }

        // expressions with subexpressions
        Expr::UnaryNot(x)
        | Expr::Keyword(_, _, x)
        | Expr::ValueWithUnit(x, _)
        | Expr::Spread(x) => {
            reparse_garbage_paths_in_expr(x, working_set);
        }
        Expr::BinaryOp(x, _, y) => {
            reparse_garbage_paths_in_expr(x, working_set);
            reparse_garbage_paths_in_expr(y, working_set);
        }
        Expr::Range(x, y, z, _) => {
            for expr in [x, y, z] {
                let Some(expr) = expr else {
                    continue;
                };
                reparse_garbage_paths_in_expr(expr, working_set);
            }
        }
        Expr::Call(call) => call
            .arguments
            .iter_mut()
            .filter_map(|a| a.expression_mut())
            .for_each(|e| reparse_garbage_paths_in_expr(e, working_set)),
        Expr::ExternalCall(x, es, _) => {
            reparse_garbage_paths_in_expr(x, working_set);
            for e in es {
                let (ExternalArgument::Regular(expr) | ExternalArgument::Spread(expr)) = e;
                reparse_garbage_paths_in_expr(expr, working_set);
            }
        }
        Expr::Subexpression(block_id) | Expr::Block(block_id) | Expr::Closure(block_id) => {
            reparse_garbage_paths_in_block(*block_id, working_set);
        }
        Expr::List(xs) | Expr::StringInterpolation(xs) => {
            xs.iter_mut()
                .for_each(|e| reparse_garbage_paths_in_expr(e, working_set));
        }
        Expr::Table(xs, yss) => {
            xs.iter_mut()
                .chain(yss.iter_mut().flatten())
                .for_each(|e| reparse_garbage_paths_in_expr(e, working_set));
        }
        Expr::MatchBlock(ms) => {
            ms.iter_mut()
                .for_each(|(_, e)| reparse_garbage_paths_in_expr(e, working_set));
        }
        Expr::Record(rs) => {
            rs.iter_mut().for_each(|r| match r {
                RecordItem::Pair(x, y) => {
                    reparse_garbage_paths_in_expr(x, working_set);
                    reparse_garbage_paths_in_expr(y, working_set);
                }
                RecordItem::Spread(_, x) => {
                    reparse_garbage_paths_in_expr(x, working_set);
                }
            });
        }
        Expr::FullCellPath(path) => {
            reparse_garbage_paths_in_expr(&mut path.head, working_set);
        }

        // the rest (left here to ensure all are checked in case of an update)
        Expr::Bool(_)
        | Expr::Int(_)
        | Expr::Float(_)
        | Expr::Binary(_)
        | Expr::Var(_)
        | Expr::VarDecl(_)
        | Expr::Operator(_)
        | Expr::RowCondition(_)
        | Expr::DateTime(_)
        | Expr::Filepath(_)
        | Expr::Directory(_)
        | Expr::GlobPattern(_)
        | Expr::String(_)
        | Expr::CellPath(_)
        | Expr::ImportPattern(_)
        | Expr::Overlay(_)
        | Expr::Signature(_)
        | Expr::Nothing
        | Expr::Garbage => {}
    }
}

/// Get the name and [`DeclId`] for a given [`Command`].
fn get_cmd_decl_id(command: &impl Command, working_set: &StateWorkingSet<'_>) -> DeclId {
    let name = command.name();
    working_set
        .find_decl(name.as_bytes())
        .unwrap_or_else(|| panic!("command {name} not defined"))
}

/// Get calls of a particular [`DeclId`] inside a given [`Pipeline`]s' elements.
fn calls_in_pipelines(
    pipelines: &mut [Pipeline],
    decl_id: DeclId,
) -> impl Iterator<Item = &mut Expression> {
    pipelines
        .iter_mut()
        .flat_map(|p| p.elements.iter_mut())
        .filter_map(move |pe| {
            if let PipelineElement::Expression(_, expr) = pe
                && matches!(&expr.expr, Expr::Call(call) if call.decl_id == decl_id)
            {
                Some(expr)
            } else {
                None
            }
        })
}

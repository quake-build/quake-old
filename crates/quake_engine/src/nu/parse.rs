use nu_parser::parse_value;
use nu_protocol::ast::{Block, Expr, Expression, Pipeline, PipelineElement};
use nu_protocol::engine::{Command, StateWorkingSet};
use nu_protocol::{Category, DeclId, Spanned, SyntaxShape};

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
/// `def-task` commands are [defined with a signature](super::commands::DefTask::signature) that, in
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
    let span = call_expr.span;

    match_expr!(Expr::Call(call), call_expr);

    // extract flag values
    let flags = TaskFlags {
        concurrent: call.has_flag("concurrent"),
    };

    // iterate only over positional args (which excludes any flags)
    let mut arguments = call.positional_iter_mut();

    match_expr!(Expr::String(name), name_span, arguments.next().unwrap());

    // extract the "signature" (actually a list<any>) to get its span
    let sig_arg = arguments.next().unwrap();

    // replace the bogus list with an empty one
    sig_arg.expr = Expr::List(Vec::new());

    // re-parse signature span into actual signature
    working_set.enter_scope();
    match_expr!(
        Expr::Signature(mut signature),
        parse_value(working_set, sig_arg.span, &SyntaxShape::Signature)
    );

    // update signature details with additional information
    signature.name = name.to_owned();
    signature.category = Category::Custom(QUAKE_CATEGORY.to_owned());

    // reparse bodies(s) with their signature now applied
    let mut bodies = Vec::with_capacity(2);
    for arg in arguments.by_ref().take(2) {
        // erase the block as we will be replacing it
        match_expr!(Expr::Closure(block_id), arg, else { break; });
        *working_set.get_block_mut(*block_id) = Block::new();

        // remove any errors inside before we reparse
        working_set
            .parse_errors
            .retain(|err| !arg.span.contains_span(err.span()));

        // reparse and replace the closure
        *arg = parse_value(working_set, arg.span, &SyntaxShape::Closure(None));

        // extract the expression and add the new block id
        match_expr!(Expr::Closure(block_id), arg, else { panic!("bad closure reparse"); });
        bodies.push(*block_id);

        let block = working_set.get_block_mut(*block_id);

        // add `$quake_scope` to the block's captures
        block.captures.push(QUAKE_SCOPE_VARIABLE_ID);

        // update the signature
        block.signature = signature.clone();
    }

    working_set.exit_scope();

    debug_assert!(
        arguments.next().is_none(),
        "extraneous args in def-task call"
    );

    // determine block IDs for metadata
    let (run_body, decl_body) = match bodies[..] {
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

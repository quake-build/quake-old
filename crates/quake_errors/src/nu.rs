use std::ops::Deref;

use miette::{Diagnostic, Report};
use nu_protocol::{ParseError, ShellError, Span};

pub trait IntoParseError {
    fn into_parse_error(self) -> ParseError;
}

impl IntoParseError for Report {
    fn into_parse_error(self) -> ParseError {
        diagnostic_to_parse_error(&*self)
    }
}

fn diagnostic_to_parse_error(diag: &dyn Diagnostic) -> ParseError {
    let error = diag.to_string();

    let label = diag.labels().and_then(|mut ls| ls.next());
    let help = diag.help();

    match (label, help) {
        (Some(label), Some(help)) => ParseError::LabeledErrorWithHelp {
            error,
            label: label.label().map(ToString::to_string).unwrap_or_default(),
            span: convert_span(label.inner()),
            help: help.to_string(),
        },
        (Some(label), None) => ParseError::LabeledError(
            error,
            label.label().map(ToString::to_string).unwrap_or_default(),
            convert_span(label.inner()),
        ),
        _ => ParseError::InternalError(error, Span::unknown()),
    }
}

pub trait IntoShellError {
    fn into_shell_error(self) -> ShellError;
}

impl IntoShellError for Report {
    fn into_shell_error(self) -> ShellError {
        diagnostic_to_shell_error(self.deref())
    }
}

fn diagnostic_to_shell_error(diag: &dyn Diagnostic) -> ShellError {
    let label = diag.labels().and_then(|mut ls| ls.next());
    ShellError::GenericError {
        error: diag.to_string(),
        msg: label
            .as_ref()
            .and_then(|l| l.label().map(ToString::to_string))
            .unwrap_or_default(),
        span: label.map(|l| convert_span(l.inner())),
        help: diag.help().map(|h| format!("{h}")),
        inner: diag
            .diagnostic_source()
            .map(|d| vec![diagnostic_to_shell_error(d)])
            .unwrap_or_default(),
    }
}

#[inline(always)]
fn convert_span(span: &miette::SourceSpan) -> Span {
    Span::new(span.offset(), span.offset() + span.len())
}

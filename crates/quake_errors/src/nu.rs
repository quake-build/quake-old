use miette::{Diagnostic, ErrReport};
use nu_protocol::Span;

pub use nu_protocol::{ParseError, ShellError};

use crate::QuakeDiagnostic;

pub type ParseResult<T> = Result<T, ParseError>;
pub type ShellResult<T> = Result<T, ShellError>;

pub trait IntoParseResult<T> {
    fn into_parse_result(self) -> ParseResult<T>;
}

impl<T, E: IntoParseError> IntoParseResult<T> for Result<T, E> {
    #[inline(always)]
    fn into_parse_result(self) -> ParseResult<T> {
        self.map_err(IntoParseError::into_parse_error)
    }
}

pub trait IntoParseError {
    fn into_parse_error(self) -> ParseError;
}

impl<E: QuakeDiagnostic> IntoParseError for E {
    #[inline(always)]
    fn into_parse_error(self) -> ParseError {
        diagnostic_to_parse_error(&self)
    }
}

impl IntoParseError for ErrReport {
    #[inline(always)]
    fn into_parse_error(self) -> ParseError {
        diagnostic_to_parse_error(&*self)
    }
}

#[inline(always)]
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

pub trait IntoShellResult<T> {
    fn into_shell_result(self) -> ShellResult<T>;
}

impl<T, E: IntoShellError> IntoShellResult<T> for Result<T, E> {
    #[inline(always)]
    fn into_shell_result(self) -> ShellResult<T> {
        self.map_err(IntoShellError::into_shell_error)
    }
}

pub trait IntoShellError {
    fn into_shell_error(self) -> ShellError;
}

impl<E: QuakeDiagnostic> IntoShellError for E {
    #[inline(always)]
    fn into_shell_error(self) -> ShellError {
        diagnostic_to_shell_error(&self)
    }
}

impl IntoShellError for ErrReport {
    #[inline(always)]
    fn into_shell_error(self) -> ShellError {
        diagnostic_to_shell_error(&*self)
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
        help: diag.help().map(|h| h.to_string()),
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

pub mod errors;

use std::ops::Deref;

pub use miette::{miette, Context, IntoDiagnostic};

pub type Result<T> = miette::Result<T>;

pub trait IntoShellError {
    fn into_shell_error(self) -> nu_protocol::ShellError;
}

impl IntoShellError for miette::Report {
    fn into_shell_error(self) -> nu_protocol::ShellError {
        diagnostic_to_shell_error(self.deref())
    }
}

fn diagnostic_to_shell_error(diag: &dyn miette::Diagnostic) -> nu_protocol::ShellError {
    let label = diag.labels().and_then(|mut ls| ls.next());
    nu_protocol::ShellError::GenericError(
        format!("{diag}"),
        label
            .as_ref()
            .and_then(|l| l.label().map(ToOwned::to_owned))
            .unwrap_or_default(),
        label.map(|l| nu_protocol::Span::new(l.offset(), l.offset() + l.len())),
        diag.help().map(|h| format!("{h}")),
        diag.diagnostic_source()
            .map(|d| vec![diagnostic_to_shell_error(d)])
            .unwrap_or_default(),
    )
}

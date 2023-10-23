use miette::Diagnostic;
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
#[error("I/O error")]
#[diagnostic(code(system::io))]
pub struct IO {
    #[from]
    pub source: std::io::Error,
}

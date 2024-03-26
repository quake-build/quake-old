pub use miette::{self, Context, IntoDiagnostic};

pub mod errors;

mod nu;
pub use nu::*;

mod macros;

pub type Result<T> = miette::Result<T>;

pub type Error = miette::Error;

pub(crate) trait QuakeDiagnostic: miette::Diagnostic {}

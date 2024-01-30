mod nu;
pub use nu::*;

pub mod errors;

pub use miette::{miette, Context, IntoDiagnostic};

pub type Result<T> = miette::Result<T>;

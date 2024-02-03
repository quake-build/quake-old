pub use ::miette::{miette, Context, IntoDiagnostic};

mod nu;
pub use nu::*;

pub mod errors;

pub type Result<T> = miette::Result<T>;

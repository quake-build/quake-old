pub use miette::{self, Context, IntoDiagnostic};

pub mod errors;

mod nu;
pub use nu::*;

mod macros;
pub use macros::private;

pub type Result<T> = miette::Result<T>;

pub mod errors;

pub use miette::{Context, IntoDiagnostic};

pub type Result<T> = miette::Result<T>;

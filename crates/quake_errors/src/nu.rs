pub use nu_protocol::{ParseError, ShellError};

/// Result type for Nu [`ParseError`]s.
pub type ParseResult<T> = Result<T, ParseError>;

/// Result type for Nu [`ParseError`]s.
pub type ShellResult<T> = Result<T, ShellError>;

const INTERNAL_FAILURE_MSG: &str = "quake internal failure (likely a bug, plase report)";

pub trait ShellErrorExt {
    /// Construct a [`ShellError`] that represents a quake internal failure that
    /// will be filtered out when errors are reported by the engine.
    ///
    /// This is useful when implementing nushell traits such as
    /// [`Command`](::nu_protocol::engine::Command), whose methods such as
    /// [`run`](::nu_protocol::engine::Command::run) are required to return a
    /// result.
    fn quake_internal() -> Self;

    /// Check whether or not this is a quake internal failure (see
    /// [`ShellErrorExt::quake_internal`]).
    fn is_quake_internal(&self) -> bool;
}

impl ShellErrorExt for ShellError {
    fn quake_internal() -> ShellError {
        ShellError::NushellFailed {
            msg: INTERNAL_FAILURE_MSG.to_owned(),
        }
    }

    fn is_quake_internal(&self) -> bool {
        matches!(self, ShellError::NushellFailed { msg } if msg == INTERNAL_FAILURE_MSG)
    }
}

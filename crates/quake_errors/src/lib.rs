#![feature(try_trait_v2)]

use std::convert::Infallible;
use std::fmt::Debug;
use std::ops::{ControlFlow, FromResidual, Try};
use std::process::{ExitCode, Termination};

use thiserror::Error;

use quake_log::log_fatal;

pub use anyhow::{self, Context as ErrorContext};
pub use miette::{self, Context as DiagnosticContext, ErrReport, IntoDiagnostic};

mod macros;
mod nu;

pub mod errors;

pub use macros::*;
pub use nu::*;

/// Result type used internally by quake for diagnostics emitted to the user via
/// stderr.
///
/// See [`Result`] for a higher level result type emitted by quake.
pub type DiagResult<T> = miette::Result<T>;

/// Result type as returned by the quake engine.
///
/// This type can be converted into [`CliResult`], TODO finish
///
/// See also: [`EngineError`]
pub type EngineResult<T> = Result<T, EngineError>;

/// Wrapper type around [`EngineResult`] to facilitate termination.
pub struct CliResult<T = ()>(EngineResult<T>);

impl<T> CliResult<T> {
    pub const fn new(inner: EngineResult<T>) -> Self {
        Self(inner)
    }

    pub const fn inner(&self) -> &EngineResult<T> {
        &self.0
    }
}

impl CliResult {
    pub const fn success() -> Self {
        Self(Ok(()))
    }
}

impl<T> From<EngineResult<T>> for CliResult<T> {
    fn from(value: EngineResult<T>) -> Self {
        Self(value)
    }
}

impl<T> Try for CliResult<T> {
    type Output = T;
    type Residual = CliResult<Infallible>;

    #[inline]
    fn from_output(output: Self::Output) -> Self {
        Self(Ok(output))
    }

    #[inline]
    fn branch(self) -> ControlFlow<Self::Residual, Self::Output> {
        match self.0 {
            Ok(val) => ControlFlow::Continue(val),
            Err(err) => ControlFlow::Break(CliResult(Err(err))),
        }
    }
}

impl<T> FromResidual<CliResult<Infallible>> for CliResult<T> {
    #[inline]
    fn from_residual(residual: CliResult<Infallible>) -> Self {
        match residual.0 {
            Err(err) => CliResult(Err(err)),
            Ok(_) => unreachable!(),
        }
    }
}

impl<T, E: Into<EngineError>> FromResidual<Result<Infallible, E>> for CliResult<T> {
    #[inline]
    fn from_residual(residual: Result<Infallible, E>) -> Self {
        match residual {
            Err(err) => CliResult(Err(err.into())),
            Ok(_) => unreachable!(),
        }
    }
}

impl Termination for CliResult {
    fn report(self) -> ExitCode {
        match self.0 {
            Ok(_) => ExitCode::SUCCESS,
            Err(error) => {
                log_fatal!(error.to_string());
                error.exit_code()
            }
        }
    }
}

/// High-level quake engine error type.
///
/// While the quake engine may report any number of diagnostics to the user via
/// stderr, from [internal errors](crate::errors) or from Nushell, this type is
/// used to report the final termination causes for the engine.
#[derive(Debug, Error)]
pub enum EngineError {
    #[error("failed to load build script")]
    LoadFailed,
    #[error("failed to parse build script")]
    ParseFailed,
    #[error("failed to evaluate build script")]
    EvalFailed,
    #[error("task failed")]
    TaskFailed { task_name: String },
    #[error("internal error: {message}")]
    Internal { message: String },
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl EngineError {
    pub fn internal(message: impl Into<String>) -> Self {
        EngineError::Internal {
            message: message.into(),
        }
    }

    pub fn exit_code(&self) -> ExitCode {
        match self {
            EngineError::LoadFailed
            | EngineError::ParseFailed
            | EngineError::EvalFailed
            | EngineError::Other { .. } => exit_codes::CAUSE_OTHER,
            EngineError::TaskFailed { .. } => exit_codes::CAUSE_USER,
            EngineError::Internal { .. } => exit_codes::CAUSE_INTERNAL,
        }
        .into()
    }
}

/// Exit codes that may be emitted by quake during runtime.
pub mod exit_codes {
    /// Cause for errors likely due have arise an issue caused by the user
    /// invoking quake (e.g. I/O errors).
    pub const CAUSE_USER: u8 = 1;

    /// Cause for errors that may have been emitted either as a result of user
    /// error, a bad build script, or simply an enternal error.
    pub const CAUSE_OTHER: u8 = 127;

    /// Cause for errors that originated internally--almost always a bug.
    pub const CAUSE_INTERNAL: u8 = 255;
}

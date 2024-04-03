//! quake-specific reimplementations and exports of error-related macros for
//! `anyhow` and `miette`.
//!
//! For general, user-facing error reporting:
//! - [`error!`]: re-export of [`anyhow::anyhow!`]
//! - [`bail!`]: re-export of [`anyhow::bail!`]
//!
//! For diagnostics reporting:
//! - [`diag_error`]: [`miette::miette!`] with `quake::other` error code.
//! - [`diag_bail!`]: [`miette::bail!`] with `quake::other` error code.
//!
//! Typed error codes are generally preferred over ad-hoc ones--see the
//! [`errors`](crate::errors) module.

pub use crate::anyhow::anyhow as error;

#[macro_export]
macro_rules! bail {
    ($($tt:tt)*) => {
        return $crate::EngineResult::Err($crate::EngineError::Other($crate::error!($($tt)*)));
    }
}

/// Wrapper of [`miette::miette!`] with the `quake::other` error code.
///
/// This should be used sparingly--it's generally better to add a typed error
/// with a specific error code. Alternatively, you can override the error code
/// with this macro like so:
///
/// ```
/// # use miette::Severity;
/// # use quake_errors::error;
/// # let err =
/// diag_error!(
///     code = "quake::project::sentient",
///     severity = Severity::Advice,
///     "the project has become sentient",
/// );
/// # assert_eq!(err.code().unwrap().to_string(), "quake::project::sentient");
/// # assert_eq!(err.severity(), Some(Severity::Advice));
/// # assert_eq!(err.to_string(), "the project has become sentient");
/// ```
#[macro_export]
macro_rules! diag_error {
    ($($key:ident = $value:expr,)* $fmt:literal $($arg:tt)*) => {
        $crate::miette::miette!(
            code = $crate::errors::QUAKE_OTHER_ERROR_CODE,
            $($key = $value,)*
            $fmt
            $($arg)*
        )
    };
}

/// Equivalent of [`miette::bail!`] with the `quake::other` error code (see
/// also: [`error!`]).
///
/// This should be used sparingly--it's generally better to add a typed error
/// with a specific error code. Alternatively, you can override the error code
/// with this macro like so:
///
/// ```
/// # use quake_errors::{bail, Result};
/// fn run_task() -> DiagResult<()> {
///     bail_diag!(
///         code = "quake::task::on_fire",
///         "the task is {task_state}",
///          task_state = "on fire?!"
///     );
///     Ok(())
/// }
/// # let err = run_task().unwrap_err();
/// # assert_eq!(err.code().unwrap().to_string(), "quake::task::on_fire");
/// # assert_eq!(err.to_string(), "the task is on fire?!");
/// ```
#[macro_export]
macro_rules! diag_bail {
    ($($key:ident = $value:expr,)* $fmt:literal $($arg:tt)*) => {
        return ::core::result::Result::Err(
            $crate::diag_error!(
                $($key = $value,)*
                $fmt
                $($arg)*
            )
        );
    };
}

#[cfg(test)]
mod tests {
    use miette::Severity;

    use crate::errors::QUAKE_OTHER_ERROR_CODE;

    #[test]
    fn test_error_macro() {
        let error = diag_error!("failed");
        assert_eq!(error.to_string(), "failed");
        assert_eq!(error.code().unwrap().to_string(), QUAKE_OTHER_ERROR_CODE);

        let error = diag_error!(
            code = "quake::task::on_fire",
            severity = Severity::Advice,
            "task is ablaze"
        );
        assert_eq!(error.to_string(), "task is ablaze");
        assert_eq!(error.severity(), Some(Severity::Advice));
        assert_eq!(error.code().unwrap().to_string(), "quake::task::on_fire");
    }

    #[test]
    #[allow(unreachable_code)]
    fn test_bail_macro() {
        let err = (|| {
            diag_bail!("failed");
            Ok(())
        })()
        .unwrap_err();
        assert_eq!(err.to_string(), "failed");
        assert_eq!(err.code().unwrap().to_string(), QUAKE_OTHER_ERROR_CODE);

        let err = (|| {
            diag_bail!(
                code = "quake::task::on_fire",
                severity = Severity::Advice,
                "task is ablaze"
            );
            Ok(())
        })()
        .unwrap_err();
        assert_eq!(err.to_string(), "task is ablaze");
        assert_eq!(err.code().unwrap().to_string(), "quake::task::on_fire");
    }
}

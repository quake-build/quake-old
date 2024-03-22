pub use {quake_errors as errors, quake_log as log};

mod macros;

pub mod metadata;
pub mod project;
pub mod utils;

/// Build script names quake will automatically detect (case-sensitive), in
/// descending precedence.
pub const BUILD_SCRIPT_NAMES: &[&str] = &["build.quake", "build.quake.nu"];

pub mod exit_codes {
    pub const TASK_RUN_FAIL: i32 = 1;
    pub const TASK_DECL_FAIL: i32 = 2;
    pub const LOAD_FAIL: i32 = 3;

    pub const INTERNAL_ERROR: i32 = 255;
}

pub mod prelude {
    pub use quake_errors::*;
    pub use quake_log::{log_error, log_fatal, log_info, log_warning, panic_bug};

    pub use crate::project::*;
    pub use crate::{exit_codes, match_expr};
}

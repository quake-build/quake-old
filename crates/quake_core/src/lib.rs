pub use ::quake_errors as errors;

pub mod metadata;
pub mod project;
pub mod utils;

/// Build script names quake will automatically detect (case-sensitive), in
/// descending precedence.
pub const BUILD_SCRIPT_NAMES: &[&str] = &["build.quake", "build.quake.nu"];

pub mod exit_codes {
    pub const LOAD_FAIL: i32 = 100;
    pub const TASK_DECL_FAIL: i32 = 101;
    pub const TASK_RUN_FAIL: i32 = 102;
}

pub mod prelude {
    pub use ::quake_errors::*;

    pub use crate::exit_codes;
    pub use crate::project::*;
    pub use crate::utils::{print_error, print_info, print_warning};
}

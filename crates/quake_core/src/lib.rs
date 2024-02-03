pub mod prelude;
pub mod project;

/// Build script names quake will automatically detect (case-sensitive), in
/// descending precedence.
pub const BUILD_SCRIPT_NAMES: &[&str] = &["build.quake", "build.quake.nu"];

pub mod exit_codes {
    pub const LOAD_FAIL: i32 = 100;
    pub const TASK_DECL_FAIL: i32 = 101;
    pub const TASK_RUN_FAIL: i32 = 102;
}

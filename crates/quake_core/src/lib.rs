pub mod prelude;

pub mod project;

/// Build script names quake will automatically detect (case-sensitive), in
/// descending precedence.
pub const BUILD_SCRIPT_NAMES: &[&str] = &[
    "build.quake",
    "build.quake.nu",
    "Quakefile",
    "quakefile",
    "QUAKE",
];

use miette::Diagnostic;
use thiserror::Error;

#[derive(Debug, Clone, Error, Diagnostic)]
#[error("Failed to locate project")]
#[diagnostic(code(quake::project::not_found))]
pub struct ProjectNotFound;

#[derive(Debug, Clone, Error, Diagnostic)]
#[error("Build script not found")]
#[diagnostic(
    code(quake::project::build_script_not_found),
    help("Add a `build.quake` file to the project root")
)]
pub struct BuildScriptNotFound;

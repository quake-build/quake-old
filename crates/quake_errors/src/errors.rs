use miette::Diagnostic;
use nu_protocol::Span;
use thiserror::Error;

#[derive(Debug, Clone, Error, Diagnostic)]
#[error("Failed to locate project")]
#[diagnostic(code(quake::project::not_found))]
pub struct ProjectNotFound;

#[derive(Debug, Clone, Error, Diagnostic)]
#[error("Build script not found")]
#[diagnostic(
    code(quake::project::missing_build_script),
    help("Add a `build.quake` file to the project root")
)]
pub struct BuildScriptNotFound;

// TODO add "did you mean?" or list available tasks
#[derive(Debug, Clone, Error, Diagnostic)]
#[error("Task not found: {task}")]
#[diagnostic(code(quake::task::not_found))]
pub struct TaskNotFound {
    pub task: String,
}

// TODO add optional suggestions for intended scope blocks
#[derive(Debug, Clone, Error, Diagnostic)]
#[error("Undefined scope")]
#[diagnostic(
    code(quake::scope::undefined),
    help("Did you mean to evaluate this command inside of a special scope block?")
)]
pub struct UndefinedScope {
    pub span: Span,
}

#[derive(Debug, Clone, Error, Diagnostic)]
#[error("Scope mismatch")]
#[diagnostic(
    code(quake::scope::mismatch),
    help("Did you mean to evaluate this command inside of a different special scope block?")
)]
pub struct ScopeMismatch {
    pub span: Span,
}

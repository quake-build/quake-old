use miette::Diagnostic;
use nu_protocol::Span;
use thiserror::Error;

#[derive(Debug, Clone, Error, Diagnostic)]
#[error("Project not found in directory")]
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
#[error("Task not found: {name}")]
#[diagnostic(code(quake::task::not_found))]
pub struct TaskNotFound {
    pub name: String,
}

#[derive(Debug, Clone, Error, Diagnostic)]
#[error("Task already defined: {name}")]
#[diagnostic(code(quake::task::duplicate_definition))]
pub struct TaskDuplicateDefinition {
    pub name: String,
    #[label("first defined here")]
    pub existing_span: Span,
}

#[derive(Debug, Clone, Error, Diagnostic)]
#[error("Unknown scope")]
#[diagnostic(
    code(quake::scope::unknown),
    help("Did you mean to evaluate this command inside of a special scope block? (e.g. def-task)")
)]
pub struct UnknownScope {
    #[label("command used here")]
    pub span: Span,
}

#[derive(Debug, Clone, Error, Diagnostic)]
#[error("Attempt to define nested task scopes")]
#[diagnostic(
    code(quake::scope::no_nested_scopes),
    help("Define this task in the outer scope instead, or use `subtask`")
)]
pub struct NestedScopes {
    #[label("command used here")]
    pub span: Span,
}

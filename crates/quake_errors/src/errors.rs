use miette::Diagnostic;
use nu_protocol::Span;
use thiserror::Error;

pub const QUAKE_OTHER_ERROR_CODE: &str = "quake::other";

#[derive(Debug, Clone, Error, Diagnostic)]
#[error("Project not found in directory")]
#[diagnostic(code(quake::project_not_found))]
pub struct ProjectNotFound;

#[derive(Debug, Clone, Error, Diagnostic)]
#[error("Build script not found")]
#[diagnostic(
    code(quake::missing_build_script),
    help("Add a `build.quake` file to the project root")
)]
pub struct BuildScriptNotFound;

// TODO add "did you mean?" or list available tasks
#[derive(Debug, Clone, Error, Diagnostic)]
#[error("Task not found: {name}")]
#[diagnostic(code(quake::task::not_found))]
pub struct TaskNotFound {
    pub name: String,
    #[label("task referenced here")]
    pub span: Option<Span>,
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
#[error("Task cannot be depended upon")]
#[diagnostic(code(quake::task::cannot_depend))]
pub struct TaskCannotDepend {
    pub name: String,
    #[label("task referenced here")]
    pub span: Span,
}

#[derive(Debug, Clone, Error, Diagnostic)]
#[error("Unknown scope")]
#[diagnostic(
    code(quake::task::unknown_scope),
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

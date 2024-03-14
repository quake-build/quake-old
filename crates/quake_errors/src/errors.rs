use miette::Diagnostic;
use nu_protocol::Span;
use thiserror::Error;

pub const QUAKE_OTHER_ERROR_CODE: &str = "quake::other";

pub(crate) trait QuakeDiagnostic: Diagnostic {}

macro_rules! make_error {
    ($name:ident, $item:item) => {
        #[derive(Debug, Clone, Error, Diagnostic)]
        $item

        impl QuakeDiagnostic for $name {}
    }
}

macro_rules! make_errors {
    () => {};
    ($(#[$attr:meta])* $vis:vis struct $name:ident; $($rest:tt)*) => {
        make_error!($name, $(#[$attr])* $vis struct $name;);
        make_errors!($($rest)*);
    };
    ($(#[$attr:meta])* $vis:vis struct $name:ident $(<$($params:tt)+>)? $inner:tt; $($rest:tt)*) => {
        make_error!($name, $(#[$attr])* $vis struct $name $(<$($params:tt)+>)? $inner;);
        make_errors!($($rest)*);
    };
    ($(#[$attr:meta])* $vis:vis struct $name:ident $(<$($params:tt)+>)? $inner:tt $($rest:tt)*) => {
        make_error!($name, $(#[$attr])* $vis struct $name $(<$($params:tt)+>)? $inner);
        make_errors!($($rest)*);
    };
    ($(#[$attr:meta])* $vis:vis enum $name:ident $(<$($params:tt)+>)? $inner:tt $($rest:tt)*) => {
        make_error!($name, $(#[$attr])* $vis enum $name $(<$($params:tt)+>)? $inner);
        make_errors!($($rest)*);
    };
}

make_errors! {
    #[error("Project not found in directory")]
    #[diagnostic(code(quake::project_not_found))]
    pub struct ProjectNotFound;

    #[error("Build script not found")]
    #[diagnostic(
        code(quake::missing_build_script),
        help("Add a `build.quake` file to the project root")
    )]
    pub struct BuildScriptNotFound;

    // TODO add "did you mean?" or list available tasks
    #[error("Task not found: {name}")]
    #[diagnostic(code(quake::task::not_found))]
    pub struct TaskNotFound {
        pub name: String,
        #[label("task referenced here")]
        pub span: Option<Span>,
    }

    #[error("Task already defined: {name}")]
    #[diagnostic(code(quake::task::duplicate_definition))]
    pub struct TaskDuplicateDefinition {
        pub name: String,
        #[label("first defined here")]
        pub existing_span: Span,
    }

    #[error("Task cannot be depended upon")]
    #[diagnostic(code(quake::task::cannot_depend))]
    pub struct TaskCannotDepend {
        pub name: String,
        #[label("task referenced here")]
        pub span: Span,
    }

    #[error("Unknown scope")]
    #[diagnostic(
        code(quake::task::unknown_scope),
        help("Did you mean to evaluate this command inside of a special scope block? (e.g. def-task)")
    )]
    pub struct UnknownScope {
        #[label("command used here")]
        pub span: Span,
    }

    #[error("Attempt to define nested task scopes")]
    #[diagnostic(
        code(quake::scope::no_nested_scopes),
        help("Define this task in the outer scope instead, or use `subtask`")
    )]
    pub struct NestedScopes {
        #[label("command used here")]
        pub span: Span,
    }
}

#[cfg(test)]
mod tests {
    use anstream::adapter::strip_str;

    use super::*;

    #[test]
    fn test_make_errors_macro() {
        macro_rules! err {
            ($expr:expr) => {
                strip_str(&format!("{:?}", ::miette::ErrReport::from($expr))).to_string()
            };
        }

        make_errors!(
            #[error("foo")]
            #[diagnostic(code(quake::foo), help("don't do that"))]
            pub struct Foo;

            #[error("bar: {message}")]
            #[diagnostic(code(quake::bar))]
            pub struct Bar {
                pub message: &'static str,
            }

            #[error("baz: {0}")]
            #[diagnostic(code(quake::baz))]
            pub(crate) struct Baz(&'static str);

            enum Baq {
                #[error("alpha baq")]
                #[diagnostic(code(quake::baq::alpha), severity(warning))]
                Alpha,
                #[error("beta baq: {0}")]
                #[diagnostic(code(quake::baq::beta))]
                Beta(u64),
            }

            #[diagnostic(transparent)]
            #[error("foobar")]
            enum FooBar {
                Foo(#[from] Foo),
                Bar(#[from] Bar),
            }
        );

        assert_eq!("quake::foo\n\n  × foo\n  help: don't do that\n", err!(Foo));
        assert_eq!("quake::bar\n\n  × bar: hi\n", err!(Bar { message: "hi" }));
        assert_eq!(
            "quake::baz\n\n  × baz: abracadabra\n",
            err!(Baz("abracadabra"))
        );
        assert_eq!("quake::baq::alpha\n\n  ⚠ alpha baq\n", err!(Baq::Alpha));
        assert_eq!(
            "quake::baq::beta\n\n  × beta baq: 42\n",
            err!(Baq::Beta(42))
        );
        assert_eq!(
            "quake::foo\n\n  × foobar\n  ╰─▶ foo\n  help: don't do that\n",
            err!(FooBar::Foo(Foo))
        );
        assert_eq!(
            "quake::bar\n\n  × foobar\n  ╰─▶ bar: a horse walked into a\n",
            err!(FooBar::Bar(Bar {
                message: "a horse walked into a"
            }))
        );
    }
}

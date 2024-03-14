macro_rules! log_macros {
    ($($($(#[$attr:meta])+)? $name:ident: $level:expr),* $(,)?) => {
        $(
            #[doc = concat!(
                "Log a [`",
                stringify!($level),
                "`](crate::",
                stringify!($level),
                ")-level message with a colored prefix to stderr.")
            ]
            $(
                #[doc = "\n"]
                $(#[$attr])+
            )?
            #[macro_export]
            macro_rules! $name {
                ($$message:literal) => {
                    use $crate::LogLevel;
                    $crate::__private::print_log($level, None, $$message);
                };
                ($$message:expr) => {
                    use $crate::LogLevel;
                    $crate::__private::print_log($level, None, &$$message);
                };
                ($$title:expr, $$message:expr) => {
                    use $crate::LogLevel;
                    $crate::__private::print_log($level, Some(&$$title), &$$message);
                };
                ($$title:expr, $$fmt:literal $$($$arg:tt)*) => {
                    use $crate::LogLevel;
                    $crate::__private::print_log($level, Some(&$$title), format!($$fmt $$($$arg)*));
                };
            }
        )*
    }
}

log_macros! {
    log_info: LogLevel::Info,
    log_warning: LogLevel::Warning,
    log_error: LogLevel::Error,
    log_fatal: LogLevel::Fatal,
}

#[macro_export]
macro_rules! panic_bug {
    ($msg:tt $(,)?) => {
        panic!($msg)
    };
    ($msg:tt, $($t:tt)*) => {
        panic!(concat!($msg, " (this is a bug)") $($t)*)
    };
}

#[doc(hidden)]
pub mod __private {
    use anstream::eprintln;
    use anstyle::{AnsiColor, Color, Style};

    use crate::LogLevel;

    const PREFIX_STYLE: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::White)));
    const MESSAGE_STYLE: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::White)));

    #[inline(always)]
    pub fn print_log(level: LogLevel, title: Option<&str>, message: &str) {
        eprintln!(
            "{PREFIX_STYLE}> {title_style}{title}:{title_style:#} {MESSAGE_STYLE}{message}",
            title = title.unwrap_or_else(|| level.name()),
            title_style = Style::new().fg_color(Some(level.color())).bold()
        );
    }
}

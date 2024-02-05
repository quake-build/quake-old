#![feature(macro_metavar_expr)]

use std::fmt::{self, Display};

mod macros;

/// Logging level ordered by severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
    Fatal,
}

impl LogLevel {
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warn",
            Self::Error => "error",
            Self::Fatal => "fatal",
        }
    }

    pub const fn color(&self) -> anstyle::Color {
        use anstyle::{AnsiColor, Color};

        match self {
            Self::Info => Color::Ansi(AnsiColor::BrightWhite),
            Self::Warning => Color::Ansi(AnsiColor::Yellow),
            Self::Error => Color::Ansi(AnsiColor::BrightRed),
            Self::Fatal => Color::Ansi(AnsiColor::Red),
        }
    }
}

impl Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

pub mod __private {
    use anstream::eprintln;
    use anstyle::{AnsiColor, Color, Style};

    use crate::LogLevel;

    const PREFIX_STYLE: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::White)));
    const MESSAGE_STYLE: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::White)));

    pub fn print_log(level: LogLevel, title: Option<&str>, message: &str) {
        eprintln!(
            "{PREFIX_STYLE}> {title_style}{title}:{title_style:#} {MESSAGE_STYLE}{message}",
            title = title.unwrap_or(level.name()),
            title_style = Style::new().fg_color(Some(level.color())).bold()
        );
    }
}

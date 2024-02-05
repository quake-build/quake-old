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

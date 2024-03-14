#[macro_export]
macro_rules! match_expr {
    ($expr:pat, $arg:expr $(,)?) => {
        $crate::match_expr!($expr, _, $arg)
    };
    ($expr:pat, $span:pat, $arg:expr $(,)?) => {
        match_expr!($expr, $span, $arg, else {
            $crate::log::panic_bug!("unexpected syntax while parsing quake syntax (this is a bug)")
        })
    };
    ($expr:pat, $arg:expr, else $else:block $(,)?) => {
        $crate::match_expr!($expr, _, $arg, else $else)
    };
    ($expr:pat, $span:pat, $arg:expr, else $else:block $(,)?) => {
        let ::nu_protocol::ast::Expression { expr: $expr, span: $span, .. } = $arg else $else;
    };
}

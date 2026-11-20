use rikka_app::prelude::*;

pub(crate) fn notice(symbol: MaterialSymbol, message: impl Into<Label>, color: Color) -> Banner {
    banner(message).symbol(symbol).tint(color)
}

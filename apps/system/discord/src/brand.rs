use rikka_app::prelude::*;

const BLURPLE: u32 = 0x5865F2;

pub(crate) fn mark() -> IconSource {
    BrandSymbol::DISCORD.into()
}

pub(crate) fn blurple() -> Color {
    Color::hex(BLURPLE)
}

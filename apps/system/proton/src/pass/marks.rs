use proton_vaults::{colors, icons};
use rikka_app::prelude::*;

pub(crate) const SYMBOLS: [MaterialSymbol; 30] = [
    MaterialSymbol::HOME,
    MaterialSymbol::WORK,
    MaterialSymbol::REDEEM,
    MaterialSymbol::STOREFRONT,
    MaterialSymbol::FAVORITE,
    MaterialSymbol::PETS,
    MaterialSymbol::WORKSPACES,
    MaterialSymbol::LOCAL_FLORIST,
    MaterialSymbol::GROUP,
    MaterialSymbol::VIDEOGAME_ASSET,
    MaterialSymbol::SHOPPING_CART,
    MaterialSymbol::ECO,
    MaterialSymbol::SHIELD,
    MaterialSymbol::SPORTS_BASKETBALL,
    MaterialSymbol::CREDIT_CARD,
    MaterialSymbol::SET_MEAL,
    MaterialSymbol::MOOD,
    MaterialSymbol::LOCK,
    MaterialSymbol::FOREST,
    MaterialSymbol::STAR,
    MaterialSymbol::LOCAL_FIRE_DEPARTMENT,
    MaterialSymbol::ACCOUNT_BALANCE_WALLET,
    MaterialSymbol::BOOKMARK,
    MaterialSymbol::ICECREAM,
    MaterialSymbol::LAPTOP,
    MaterialSymbol::DATA_OBJECT,
    MaterialSymbol::MENU_BOOK,
    MaterialSymbol::INVENTORY_2,
    MaterialSymbol::SCIENCE,
    MaterialSymbol::RECEIPT_LONG,
];

const ON_DARK: [u32; 10] = [
    0xA779FF, 0xF29292, 0xF7D775, 0x91C799, 0x92B3F2, 0xEB8DD6, 0xCD5A6F, 0xE4A367, 0xE6E6E6,
    0x9EE2E6,
];

const ON_LIGHT: [u32; 10] = [
    0x8849FF, 0xFF4949, 0xCC6200, 0x006677, 0x3B88FC, 0xDD11B0, 0xCB0126, 0xFF7A00, 0x757575,
    0x149AA1,
];

pub(crate) const SHADES: usize = ON_DARK.len();

pub(crate) fn shade(at: usize) -> Color {
    let text = palette().text;
    let on_dark = text.r + text.g + text.b > 1.5;
    let shades = match on_dark {
        true => ON_DARK,
        false => ON_LIGHT,
    };
    Color::hex(shades[at.min(SHADES - 1)])
}

pub(crate) fn symbol_of(icon: i32) -> MaterialSymbol {
    icons()
        .iter()
        .position(|candidate| *candidate == icon)
        .map_or(MaterialSymbol::SHIELD_LOCK, |at| SYMBOLS[at])
}

pub(crate) fn shade_of(color: i32) -> Color {
    colors()
        .iter()
        .position(|candidate| *candidate == color)
        .map_or(palette().accent, shade)
}

pub(crate) fn badge(symbol: MaterialSymbol, tint: Color, side: f32) -> Container {
    container(icon(symbol).extent(Length::px(side * 0.5)).color(tint))
        .background(tint.with_alpha(0.16))
        .radius(side / 2.0)
        .size(side, side)
        .center()
}

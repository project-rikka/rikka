use crate::prelude::*;

pub const CARD_INSET: f32 = 10.0;

pub(crate) fn card(content: Flex, inset: Inset, border: Color) -> Container {
    container(content.padding(inset).fill_width())
        .surface()
        .radius(Corner::Panel)
        .border(1.0, border)
        .fill_width()
}

#[must_use]
pub fn chevron(open: bool) -> MaterialSymbol {
    match open {
        true => MaterialSymbol::EXPAND_LESS,
        false => MaterialSymbol::EXPAND_MORE,
    }
}

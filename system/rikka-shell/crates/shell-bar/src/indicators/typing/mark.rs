use rikka_ime::Property;
use rikka_ui::{Align, Justify, Length, Styled, TextStyled, View, text};
use rikka_widgets::row;

const SIDE: f32 = 19.0;
const TYPE: f32 = 15.0;

#[must_use]
pub(crate) fn badge(current: &Property) -> Option<impl View> {
    let badge = badge_text(current)?;
    Some(
        row! { text(badge).text_size(TYPE).medium().no_wrap() }
            .justify(Justify::Center)
            .align_items(Align::Center)
            .min_width(Length::px(SIDE))
            .height(Length::px(SIDE)),
    )
}

fn badge_text(current: &Property) -> Option<String> {
    match current.badge() {
        Some(badge) => Some(badge.to_owned()),
        None => current.label.chars().next().map(String::from),
    }
}

use discord_core::MessageId;
use rikka_app::prelude::*;
use rikka_ui::{Element, Handler};

use crate::chat::Chat;
use crate::conversation::overlay::{Overlay, PickerTarget, Tab};
use crate::conversation::widgets::tap;

const BUTTON: f32 = 32.0;

#[derive(Clone, Copy)]
pub(crate) struct Allowed {
    pub(crate) react: bool,
    pub(crate) reply: bool,
    pub(crate) edit: bool,
}

pub(crate) fn bar(message: &MessageId, allowed: Allowed) -> Element {
    let mut buttons = Vec::new();
    if allowed.react {
        let id = *message;
        buttons.push(button_of(
            MaterialSymbol::ADD_REACTION,
            Handler::new(move |overlay: &mut Overlay, _: Modifiers| {
                overlay.open_picker(PickerTarget::React(id), Tab::Emoji)
            }),
        ));
    }
    if allowed.reply {
        let id = *message;
        buttons.push(button_of(
            MaterialSymbol::REPLY,
            Handler::new(move |chat: &mut Chat, _: Modifiers| chat.reply_to(&id)),
        ));
    }
    if allowed.edit {
        let id = *message;
        buttons.push(button_of(
            MaterialSymbol::EDIT,
            Handler::new(move |chat: &mut Chat, _: Modifiers| chat.start_edit(&id)),
        ));
    }
    let id = *message;
    buttons.push(button_of(
        MaterialSymbol::MORE_HORIZ,
        Handler::new(move |overlay: &mut Overlay, modifiers: Modifiers| {
            overlay.open_menu_below(id, modifiers.shift)
        }),
    ));
    container(row_of(buttons).gap(Space::HAIR))
        .padding(Inset::all(Space::HAIR))
        .background(palette().surface)
        .border(1.0, palette().outline)
        .radius(Corner::Item)
        .any()
}

fn button_of(symbol: MaterialSymbol, press: Handler<(Modifiers,)>) -> Element {
    tap(
        container(
            icon(symbol)
                .extent(Length::px(20.0))
                .color(palette().text_muted),
        )
        .center()
        .size(Length::px(BUTTON), Length::px(BUTTON)),
        press,
    )
}

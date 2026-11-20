use rikka_app::prelude::*;

use crate::chat::Chat;
use crate::settings::Section;

pub(crate) fn bind(window: Window<Chat>) -> Window<Chat> {
    window
        .on_command(Command::Preferences, |chat: &mut Chat| {
            chat.open_settings(Section::default());
        })
        .on_chord(Key::Char('k'), Modifiers::CONTROL, Chat::toggle_switcher)
        .on_chord(Key::ArrowUp, Modifiers::ALT, |chat: &mut Chat| {
            chat.step_channel(-1);
        })
        .on_chord(Key::ArrowDown, Modifiers::ALT, |chat: &mut Chat| {
            chat.step_channel(1);
        })
        .on_chord(
            Key::Char('m'),
            Modifiers::CONTROL | Modifiers::SHIFT,
            Chat::toggle_mute,
        )
        .on_chord(
            Key::Char('d'),
            Modifiers::CONTROL | Modifiers::SHIFT,
            Chat::toggle_deafen,
        )
        .on_chord(Key::Escape, Modifiers::NONE, Chat::escape)
}

use rikka_app::prelude::*;

use crate::library::Library;
use crate::music::Music;
use crate::player::Player;

pub(crate) fn bind(window: Window<Music>) -> Window<Music> {
    window
        .on_command(Command::Back, Music::back)
        .on_command(Command::Forward, Music::forward)
        .on_command(Command::Find, Music::find)
        .on_command(Command::Refresh, Library::refresh)
        .on_command(Command::Preferences, |music: &mut Music| {
            music.open_settings(true);
        })
        .on_chord(Key::Char('l'), Modifiers::CONTROL, Music::find)
        .on_chord(Key::Space, Modifiers::NONE, Player::toggle)
        .on_chord(Key::ArrowRight, Modifiers::CONTROL, Player::next)
        .on_chord(Key::ArrowLeft, Modifiers::CONTROL, Player::previous)
        .on_chord(Key::ArrowRight, Modifiers::SHIFT, |player: &mut Player| {
            player.seek_by(true);
        })
        .on_chord(Key::ArrowLeft, Modifiers::SHIFT, |player: &mut Player| {
            player.seek_by(false);
        })
        .on_chord(Key::ArrowUp, Modifiers::CONTROL, |player: &mut Player| {
            player.step_volume(true);
        })
        .on_chord(Key::ArrowDown, Modifiers::CONTROL, |player: &mut Player| {
            player.step_volume(false);
        })
        .on_chord(Key::Char('s'), Modifiers::CONTROL, Player::toggle_shuffle)
        .on_chord(Key::Char('r'), Modifiers::CONTROL, Player::cycle_repeat)
}

use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::accounts::{saved, services};
use crate::music::Music;

pub(super) fn view(music: &Music) -> impl View {
    column! {
        if let Some(error_message) = music.error_message() {
            text(error_message.to_owned()).caption().danger().fill_width();
        }
        section(tr!("music.accounts.saved"), saved(music));
        section(tr!("music.accounts.add"), services(music.providers()));
    }
    .gap(Space::NORMAL)
    .fill_width()
}

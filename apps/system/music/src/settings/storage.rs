use std::convert::Infallible;

use rikka_app::prelude::*;
use rikka_format::format_bytes;
use rikka_types::Choice;
use rikka_widgets::{column, row};

use crate::cache_size::CacheSize;
use crate::music::Music;

const CHOICE: f32 = 220.0;

pub(super) fn view(music: &Music) -> impl View {
    let chosen = music.preferences().cache();
    let audio = music.audio().clone();
    let used = fetch_blocking(("music-cache-used", music.stored()), move || {
        Ok::<u64, Infallible>(audio.prune_and_measure())
    });
    card(
        column! {
            field(
                tr!("music.settings.cache"),
                combo(CacheSize::ALL.iter().position(|size| *size == chosen))
                    .options(CacheSize::ALL.iter().copied().map(named))
                    .on_select(|music: &mut Music, index: usize| {
                        if let Some(size) = CacheSize::ALL.get(index) {
                            music.set_cache_size(*size);
                        }
                    })
                    .width(CHOICE),
            );
            field(
                tr!("music.settings.cache-used"),
                row! {
                    async_view(used, |size: &u64| text(format_bytes(*size)).label())
                        .loading(|| spinner().extent(Length::px(16.0)));
                    button(tr!("music.settings.cache-clear"))
                        .secondary()
                        .on_press(|music: &mut Music| music.clear_cache());
                }
                .gap(Space::SNUG)
                .align_items(Align::Center),
            );
        }
        .gap(Space::SNUG)
        .fill_width(),
    )
}

fn named(size: CacheSize) -> Label {
    match size {
        CacheSize::Off => tr!("music.cache.off"),
        _ => Label::from(format_bytes(size.bytes())),
    }
}

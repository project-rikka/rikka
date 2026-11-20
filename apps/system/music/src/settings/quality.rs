use music_core::Quality;
use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::music::Music;
use crate::preferences::Preferences;

const CHOICE: f32 = 220.0;

pub(super) fn view(preferences: &Preferences) -> impl View {
    card(
        column! {
            field(
                tr!("music.settings.audio"),
                choice(preferences.audio(), Music::set_audio_quality),
            );
            field(
                tr!("music.settings.image"),
                choice(preferences.image(), Music::set_image_quality),
            );
        }
        .gap(Space::SNUG)
        .fill_width(),
    )
}

fn choice(chosen: Quality, choose: fn(&mut Music, Quality)) -> impl View {
    combo(Quality::ALL.iter().position(|quality| *quality == chosen))
        .options(Quality::ALL.map(named))
        .on_select(move |music: &mut Music, index: usize| {
            if let Some(quality) = Quality::ALL.get(index) {
                choose(music, *quality);
            }
        })
        .width(CHOICE)
}

fn named(quality: Quality) -> Label {
    match quality {
        Quality::Low => tr!("music.quality.low"),
        Quality::Standard => tr!("music.quality.standard"),
        Quality::Highest => tr!("music.quality.highest"),
    }
}

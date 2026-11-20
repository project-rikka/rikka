#![cfg_attr(windows, windows_subsystem = "windows")]

mod art;
mod attributes;
mod caption;
mod command;
mod controls;
mod cut;
mod easing;
mod export;
mod gallery;
mod live;
mod media_previews;
mod monitor;
mod panel;
mod playback;
mod preferences;
mod probe;
mod quality;
mod reformat;
mod relink;
mod stage;
mod status;
mod timeline;
mod tool;
mod transport;
mod trimmer;
mod view;
mod window;
mod wording;

use rikka_app::prelude::*;

rikka_app::mimalloc!();

fn main() -> rikka_app::Result<()> {
    App::new("rikka-cut")
        .decodes_video()
        .computes()
        .i18n(
            rikka_i18n::embedded!()
                .embedded(rikka_effects::CATALOGS)
                .embedded(rikka_figure_editing::CATALOGS)
                .embedded(rikka_timeline::CATALOGS),
        )
        .windows(|open: LaunchRequest| window::opening(open.paths()))
        .run()
}

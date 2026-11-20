#![cfg_attr(windows, windows_subsystem = "windows")]

mod action;
mod assistant;
mod camera;
mod canvas;
mod caption;
mod checker;
mod colors;
mod command;
mod develop_menu;
mod direct;
mod figures;
mod ink;
mod new_image;
mod operation;
mod options;
mod paint;
mod panel;
mod picture;
mod pointer;
mod recent;
mod selection;
mod session;
mod start;
mod status;
mod stroke;
mod swatches;
mod tab;
mod thumbnail;
mod tool;
mod tools;
mod typing;
mod view;
mod window;
mod wording;
mod workspace;

use rikka_app::prelude::*;

const SHUTDOWN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

rikka_app::mimalloc!();

fn main() -> rikka_app::Result<()> {
    let ran = App::new("rikka-paint")
        .i18n(
            rikka_i18n::embedded!()
                .embedded(rikka_effects::CATALOGS)
                .embedded(rikka_figure_editing::CATALOGS),
        )
        .computes_in_background()
        .windows(|open: LaunchRequest| window::opening(open.paths()))
        .run();
    tab::drain(SHUTDOWN_TIMEOUT);
    ran
}

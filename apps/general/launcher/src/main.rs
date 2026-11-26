#![cfg_attr(windows, windows_subsystem = "windows")]

mod accounts;
mod discover;
mod editors;
mod format;
mod instance;
mod kinds;
mod launcher;
mod library;
mod load;
mod logs;
mod memory;
mod pictures;
mod preferences;
mod runs;
mod services;
mod settings;
mod sidebar;
mod skins;
mod target;
mod widgets;

use rikka_app::prelude::*;

use crate::launcher::Launcher;

rikka_app::mimalloc!();

fn main() -> rikka_app::Result<()> {
    App::new("rikka-launcher")
        .i18n(rikka_i18n::embedded!())
        .instance(Instance::single())
        .window(window())
        .run()
}

fn window() -> Window<Launcher> {
    Window::new(Launcher::new, launcher::view)
        .title(tr!("launcher.title"))
        .size(1240.0, 800.0)
        .min_size(900.0, 600.0)
        .on_start(Launcher::start)
}

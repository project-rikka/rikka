mod amount;
mod appearance;
mod audio;
mod backup;
mod bluetooth;
mod capture;
mod color_picker;
mod configuration;
mod controls;
mod date_time;
mod defaults;
mod displays;
mod domain;
mod following;
mod hardware;
mod host_name;
mod info_row;
mod keyboard;
mod keyring;
mod language;
mod llm;
mod lock;
mod network;
mod notice;
mod notifications;
mod page;
mod pending;
mod picker;
mod pointer;
mod power;
mod remote;
mod replay;
mod section;
mod session;
mod settings;
mod spot_reveal;
mod system;
mod typed;
mod users;
mod windows;

use rikka_app::prelude::*;

use crate::settings::Settings;

rikka_app::mimalloc!();

fn main() -> rikka_app::Result<()> {
    App::new("rikka-settings")
        .instance(Instance::single())
        .i18n(
            rikka_i18n::embedded!()
                .embedded(rikka_usage_bar::CATALOGS)
                .embedded(rikka_llm_accounts::CATALOGS)
                .embedded(rikka_llm_settings::CATALOGS)
                .embedded(rikka_color_formats::CATALOGS),
        )
        .window(
            Window::new(Settings::new, settings::view)
                .title(tr!("settings.title"))
                .size(880.0, 620.0)
                .min_size(720.0, 480.0)
                .on_start(Settings::start)
                .on_open(Settings::opened_with)
                .on_command(Command::Back, Settings::back)
                .on_command(Command::Forward, Settings::forward),
        )
        .run()
}

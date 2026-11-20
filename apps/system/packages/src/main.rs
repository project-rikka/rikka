mod apps;
mod apps_page;
mod cancel_guard;
mod configuration;
mod detail;
mod error;
mod generations;
mod integration;
mod job;
mod list;
mod listing;
mod manager;
mod mark;
mod pane;
mod plan;
mod platform;
mod section;
mod work;

use rikka_app::prelude::*;

use crate::manager::Manager;

rikka_app::mimalloc!();

fn main() -> rikka_app::Result<()> {
    App::new("rikka-packages")
        .instance(Instance::single())
        .i18n(rikka_i18n::embedded!())
        .window(
            Window::new(Manager::new, manager::view)
                .on_command(Command::Find, Manager::focus_search)
                .title(tr!("packages.title"))
                .size(1240.0, 760.0)
                .min_size(940.0, 580.0)
                .on_start(|manager: &mut Manager| manager.start())
                .caption(manager::caption_bar),
        )
        .run()
}

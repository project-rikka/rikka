mod format;
mod heat;
mod manager;
mod performance;
mod processes;
mod sampler;
mod tab;

use rikka_app::prelude::*;

use crate::manager::Manager;

rikka_app::mimalloc!();

fn main() -> rikka_app::Result<()> {
    App::new("rikka-task-manager")
        .instance(Instance::single())
        .i18n(rikka_i18n::embedded!())
        .window(
            Window::new(Manager::new, manager::view)
                .on_command(Command::Find, Manager::focus_search)
                .title(tr!("tasks.title"))
                .size(1180.0, 720.0)
                .min_size(880.0, 560.0)
                .on_start(|manager: &mut Manager| manager.start())
                .caption(manager::caption_bar),
        )
        .run()
}

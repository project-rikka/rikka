mod describe;
mod log;
mod updates;
mod view;

use rikka_app::prelude::*;
use rikka_ui::search::{Action, Step};

use crate::log::Log;
use crate::updates::Updates;

rikka_app::mimalloc!();

fn main() -> rikka_app::Result<()> {
    App::new("rikka-updates")
        .instance(Instance::single())
        .i18n(rikka_i18n::embedded!())
        .window(
            Window::new(Updates::new, view::view)
                .title(tr!("updates.title"))
                .size(1040.0, 680.0)
                .min_size(720.0, 440.0)
                .on_start(Updates::start)
                .on_open(Updates::opened_with)
                .caption(view::caption_bar)
                .on_command(Command::Find, Log::open_find)
                .on_command(Command::FindNext, |log: &mut Log| {
                    log.find(Action::Step(Step::Next));
                })
                .on_command(Command::FindPrevious, |log: &mut Log| {
                    log.find(Action::Step(Step::Previous));
                }),
        )
        .run()
}

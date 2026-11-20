#![recursion_limit = "256"]

mod account;
mod clients;
mod codes;
mod desk;
mod drive;
mod mail;
mod pane;
mod pass;
mod product;
mod signing;
mod wording;
mod workspace;

use rikka_app::prelude::*;

use crate::desk::Desk;
use crate::workspace::Workspace;

rikka_app::mimalloc!();

fn main() -> rikka_app::Result<()> {
    App::new("rikka-proton")
        .instance(Instance::single())
        .i18n(rikka_i18n::embedded!())
        .window(
            Workspace::COMMANDS
                .into_iter()
                .fold(Window::new(Desk::new, desk::view), |window, command| {
                    window.on_command(command, move |desk: &mut Desk| desk.command(command))
                })
                .title(tr!("proton.title"))
                .size(1320.0, 840.0)
                .min_size(980.0, 620.0)
                .on_start(|desk: &mut Desk| desk.start())
                .caption(desk::caption_bar),
        )
        .run()
}

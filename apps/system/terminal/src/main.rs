mod command;
mod group;
mod keys;
mod opening;
mod panes;
mod tab;
mod terminal;

use rikka_app::prelude::*;

use crate::opening::Opening;
use crate::terminal::window;

rikka_app::mimalloc!();

fn main() -> rikka_app::Result<()> {
    let started = LaunchRequest::from_arguments(std::env::args_os());
    App::new("rikka-terminal")
        .instance(Instance::windows(|open: LaunchRequest| {
            window(Opening::from_request(&open))
        }))
        .i18n(rikka_i18n::embedded!())
        .window(window(Opening::from_request(&started)))
        .run()
}

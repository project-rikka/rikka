mod avatar;
mod desk;
mod drive;
mod google;
mod mail;
mod passwords;
mod portal;
mod session;
mod signing;

use google_account::SavedAccount;
use rikka_account_book::Accounts;
use rikka_app::prelude::*;
use rikka_dirs::Paths;

use crate::google::Google;
use crate::session::Roots;

const FOLDER: &str = "google";
const COMMANDS: [Command; 9] = [
    Command::Refresh,
    Command::Find,
    Command::New,
    Command::Back,
    Command::Forward,
    Command::Up,
    Command::SelectAll,
    Command::Delete,
    Command::Rename,
];

rikka_app::mimalloc!();

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths = Paths::resolve()?;
    let roots = Roots {
        state: paths.state_dir().join(FOLDER),
        cache: paths.cache_dir().join(FOLDER),
    };
    let accounts = Accounts::<SavedAccount>::open(&roots.state)?;
    let window = Window::new(move || Google::new(roots, accounts), desk::view)
        .title(tr!("google.title"))
        .size(1320.0, 840.0)
        .min_size(1040.0, 640.0)
        .on_start(Google::start)
        .caption(desk::caption_bar);
    let window = COMMANDS.into_iter().fold(window, |window, command| {
        window.on_command(command, move |google: &mut Google| google.command(command))
    });
    App::new("rikka-google")
        .instance(Instance::single())
        .i18n(rikka_i18n::embedded!())
        .window(window)
        .run()?;
    rikka_web::wait_for_closed_runtimes();
    Ok(())
}

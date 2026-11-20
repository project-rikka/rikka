mod active;
mod clock;
mod drag;
mod heard;
mod hosting;
mod input;
mod looping;
mod monitor;
mod open;
mod process;
mod runner;
mod window;

use rikka_clipboard::Clipboard;

use crate::error::Result;
use crate::launch::Launch;
use crate::win32::clock::Clock;
use crate::win32::process::Ole;
use crate::win32::runner::Runner;
use crate::win32::window::handle::WindowClass;

pub(crate) fn run(launch: Launch) -> Result<()> {
    process::declare(&launch.app_name)?;
    let _ole = Ole::initialize()?;
    let class = WindowClass::register()?;
    let clock = Clock::start()?;
    open_clipboard();
    let runner = Runner::new(launch, class, clock);
    let outcome = looping::run(runner);
    rikka_ui::clipboard::close_session();
    outcome
}

fn open_clipboard() {
    match Clipboard::connect() {
        Ok(clipboard) => rikka_ui::clipboard::install_session(clipboard),
        Err(error) => tracing::warn!("failed to open the clipboard: {error}"),
    }
}

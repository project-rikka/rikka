use std::ffi::OsString;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};

use rikka_gpu_drivers::Drivers;

use crate::error::{Error, Result};
use crate::process::isolation::Isolation;

const CHANNEL: &str = "RIKKA_WEB_CHANNEL";

const WITHHELD: [&str; 7] = [
    "DISPLAY",
    "WAYLAND_SOCKET",
    "DESKTOP_STARTUP_ID",
    "XDG_ACTIVATION_TOKEN",
    "GDK_SCALE",
    "GDK_DPI_SCALE",
    "CHROME_DEVEL_SANDBOX",
];

const SWITCHES: [&str; 9] = [
    "--kiosk",
    "--no-first-run",
    "--no-default-browser-check",
    "--hide-crash-restore-bubble",
    "--ozone-platform=wayland",
    "--enable-wayland-ime",
    "--wayland-text-input-version=3",
    "--password-store=gnome-libsecret",
    "--disable-features=ChromeWhatsNewUI",
];

pub(crate) struct Launch<'a> {
    pub(crate) program: &'a Path,
    pub(crate) profile: &'a Path,
    pub(crate) display: &'a Path,
    pub(crate) channel: &'a Path,
}

pub(crate) fn launch(launch: &Launch<'_>) -> Result<Child> {
    let isolation = Isolation::plan(launch.profile)?;
    let mut data_dir = OsString::from("--user-data-dir=");
    data_dir.push(launch.profile);
    let mut command = Command::new(launch.program);
    command
        .arg(data_dir)
        .args(SWITCHES)
        .env("WAYLAND_DISPLAY", launch.display)
        .env(CHANNEL, launch.channel)
        .stdin(Stdio::null());
    for variable in WITHHELD {
        command.env_remove(variable);
    }
    if let Some(drivers) = Drivers::shipped() {
        for (name, value) in drivers.environment() {
            match value {
                Some(value) => command.env(name, value),
                None => command.env_remove(name),
            };
        }
    }
    unsafe {
        command.pre_exec(move || isolation.enter());
    }
    command.spawn().map_err(Error::Spawn)
}

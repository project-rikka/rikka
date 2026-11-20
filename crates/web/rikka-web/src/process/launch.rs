use rikka_sync::Mutex;
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Child, Command, Stdio};

use rustix::io::{FdFlags, fcntl_setfd};

use crate::error::{Error, Result};

const CHANNEL: &str = "RIKKA_GECKO_CHANNEL";

const WITHHELD: [&str; 8] = [
    "DISPLAY",
    "WAYLAND_SOCKET",
    "DESKTOP_STARTUP_ID",
    "XDG_ACTIVATION_TOKEN",
    "GDK_SCALE",
    "GDK_DPI_SCALE",
    "MOZ_HEADLESS",
    "MOZ_DISABLE_WAYLAND",
];

static SPAWNING: Mutex<()> = Mutex::new(());

#[derive(Debug)]
pub(crate) struct Launched {
    pub(crate) child: Child,
    pub(crate) channel: UnixStream,
}

pub(crate) fn launch(program: &Path, profile: &Path, display: &Path) -> Result<Launched> {
    let (channel, inherited) = UnixStream::pair().map_err(Error::Spawn)?;
    let mut command = Command::new(program);
    command
        .arg("--no-remote")
        .arg("--new-instance")
        .arg("--profile")
        .arg(profile)
        .env(CHANNEL, inherited.as_raw_fd().to_string())
        .env("WAYLAND_DISPLAY", display)
        .env("MOZ_ENABLE_WAYLAND", "1")
        .env("GDK_BACKEND", "wayland")
        .stdin(Stdio::null());
    for variable in WITHHELD {
        command.env_remove(variable);
    }
    let child = {
        let _spawning = SPAWNING.lock();
        fcntl_setfd(&inherited, FdFlags::empty()).map_err(|error| Error::Spawn(error.into()))?;
        command.spawn().map_err(Error::Spawn)?
    };
    drop(inherited);
    Ok(Launched { child, channel })
}

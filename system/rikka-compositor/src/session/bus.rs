use std::io::{self, BufRead as _, BufReader};
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use rikka_session::environment::SESSION_VARIABLES;
use rustix::process::{Pid, Signal, kill_process};

use crate::programs;

const DAEMON: &str = "dbus-daemon";

const SOCKET: &str = "bus";

const CONFIG: &str = "bus.conf";

#[derive(Debug)]
pub(crate) struct PrivateBus {
    daemon: Child,
    address: String,
    directory: PathBuf,
}

impl PrivateBus {
    pub(crate) fn start(directory: &Path) -> io::Result<PrivateBus> {
        let daemon = rikka_open::find(DAEMON).ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, format!("no {DAEMON} on PATH"))
        })?;
        std::fs::create_dir_all(directory)?;
        let socket = directory.join(SOCKET);
        match std::fs::remove_file(&socket) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }

        let mut command = Command::new(&daemon);
        command
            .arg("--nofork")
            .arg("--print-address=1")
            .arg(format!("--address=unix:path={}", socket.display()))
            .stdin(Stdio::null())
            .stdout(Stdio::piped());
        for name in SESSION_VARIABLES {
            command.env_remove(name);
        }
        match configuration(&daemon, directory)? {
            Some(config) => command.arg(format!("--config-file={}", config.display())),
            None => command.arg("--session"),
        };
        unsafe {
            command.pre_exec(|| {
                rustix::process::set_parent_process_death_signal(Some(Signal::TERM))?;
                Ok(())
            });
        }

        let mut daemon = command.spawn()?;
        let mut address = String::new();
        let read = daemon
            .stdout
            .take()
            .map(|printed| BufReader::new(printed).read_line(&mut address));
        let address = address.trim().to_owned();
        if address.is_empty() {
            let _ = daemon.kill();
            let ended = daemon.wait()?;
            return Err(match read {
                Some(Err(error)) => error,
                _ => io::Error::other(format!(
                    "{DAEMON} exited without printing an address: {ended}"
                )),
            });
        }
        tracing::info!("private session bus at {address}");
        Ok(PrivateBus {
            daemon,
            address,
            directory: directory.to_path_buf(),
        })
    }

    pub(crate) fn address(&self) -> &str {
        &self.address
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        if let Err(error) = kill_process(Pid::from_child(&self.daemon), Signal::TERM) {
            tracing::warn!("cannot stop private session bus: {error}");
        }
        if let Err(error) = self.daemon.wait() {
            tracing::warn!("cannot wait for private session bus: {error}");
        }
        for left in [SOCKET, CONFIG] {
            let path = self.directory.join(left);
            if let Err(error) = std::fs::remove_file(&path)
                && error.kind() != io::ErrorKind::NotFound
            {
                tracing::debug!("cannot remove {}: {error}", path.display());
            }
        }
    }
}

fn configuration(daemon: &Path, directory: &Path) -> io::Result<Option<PathBuf>> {
    let Some(session) = session_configuration(daemon) else {
        tracing::debug!("no session configuration found for {DAEMON}");
        return Ok(None);
    };
    let own_services = programs::prefix()
        .map(|prefix| prefix.join("share/dbus-1/services"))
        .filter(|services| services.is_dir());
    let service_dir = own_services
        .map(|services| {
            format!(
                "  <servicedir>{}</servicedir>\n",
                escaped(&services.to_string_lossy())
            )
        })
        .unwrap_or_default();
    let config = format!(
        "<!DOCTYPE busconfig PUBLIC \"-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN\"\n \
         \"http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd\">\n<busconfig>\n\
         {service_dir}  <include>{}</include>\n</busconfig>\n",
        escaped(&session.to_string_lossy())
    );
    let path = directory.join(CONFIG);
    std::fs::write(&path, config)?;
    Ok(Some(path))
}

fn session_configuration(daemon: &Path) -> Option<PathBuf> {
    let daemon = std::fs::canonicalize(daemon).ok()?;
    let prefix = daemon.parent()?.parent()?;
    Some(prefix.join("share/dbus-1/session.conf")).filter(|path| path.is_file())
}

fn escaped(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            other => escaped.push(other),
        }
    }
    escaped
}

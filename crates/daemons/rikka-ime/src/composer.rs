use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use rikka_sync::Mutex;
use zbus::blocking::Connection;
use zbus::blocking::fdo::DBusProxy;
use zbus::names::BusName;

use crate::error::{Error, Result};
use crate::names;

const PROGRAM: &str = "fcitx5";

const REPLACE: &str = "--replace";

pub struct Composer {
    connection: Connection,
    started: Mutex<Option<Process>>,
}

struct Process {
    child: Child,
    from: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Startup {
    Running,
    Started,
    Replaced,
    Absent,
}

impl Composer {
    pub fn connect() -> Result<Composer> {
        Ok(Composer {
            connection: Connection::session().map_err(Error::bus)?,
            started: Mutex::new(None),
        })
    }

    pub fn ensure_running(&self) -> Startup {
        let Some(program) = rikka_open::find(PROGRAM) else {
            tracing::info!("{PROGRAM} not installed, input method disabled");
            return Startup::Absent;
        };
        let program_path = std::fs::canonicalize(&program).unwrap_or_else(|_| program.clone());

        let mut started = self.started.lock();
        if started
            .as_mut()
            .is_some_and(|running| !matches!(running.child.try_wait(), Ok(None)))
        {
            *started = None;
        }
        if started
            .as_ref()
            .is_some_and(|running| running.from == program_path)
        {
            return Startup::Running;
        }

        let previous = started.take();
        if let Some(running) = &previous {
            tracing::info!(
                "running input method {} differs from installed {PROGRAM} {}",
                running.from.display(),
                program_path.display()
            );
        }
        let outcome = match previous.is_some() || self.is_name_owned() {
            true => Startup::Replaced,
            false => Startup::Started,
        };

        match self.start(&program, program_path, outcome) {
            Some(fresh) => {
                *started = Some(fresh);
                if let Some(replaced) = previous {
                    reap(replaced.child);
                }
                outcome
            }
            None => {
                *started = previous;
                Startup::Absent
            }
        }
    }

    fn is_name_owned(&self) -> bool {
        let owned = BusName::try_from(names::FCITX)
            .ok()
            .zip(DBusProxy::new(&self.connection).ok())
            .and_then(|(name, bus)| bus.name_has_owner(name).ok());
        owned.unwrap_or(false)
    }

    fn start(&self, program: &Path, from: PathBuf, outcome: Startup) -> Option<Process> {
        let mut command = Command::new(program);
        if outcome == Startup::Replaced {
            command.arg(REPLACE);
        }
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0);
        match command.spawn() {
            Ok(child) => {
                tracing::info!(
                    "started input method {} (pid {})",
                    from.display(),
                    child.id()
                );
                Some(Process { child, from })
            }
            Err(error) => {
                tracing::warn!("failed to start {}: {error}", program.display());
                None
            }
        }
    }
}

fn reap(mut child: Child) {
    let reaping = std::thread::Builder::new()
        .name("rikka-ime-reap".to_owned())
        .spawn(move || {
            let _reaped = child.wait();
        });
    if let Err(error) = reaping {
        tracing::warn!("failed to reap replaced input method: {error}");
    }
}

impl std::fmt::Debug for Composer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Composer").finish_non_exhaustive()
    }
}

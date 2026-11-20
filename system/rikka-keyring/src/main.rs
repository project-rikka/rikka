use std::path::PathBuf;
use std::process::ExitCode;

use rikka_keyring::{Binding, Daemon};
use rustix::process::{DumpableBehavior, Resource, Rlimit, set_dumpable_behavior, setrlimit};

const KEYRINGS: &str = "keyrings";

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("failed to keep keyring memory out of reach: {0}")]
    Harden(std::io::Error),
    #[error("no data folder: {0}")]
    Dirs(#[from] rikka_dirs::Error),
    #[error(transparent)]
    Keyring(#[from] rikka_keyring::Error),
}

fn main() -> ExitCode {
    rikka_log::install("info");
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Error> {
    harden()?;
    let folder = folder()?;
    zbus::block_on(async {
        let _served = Daemon::new(folder)
            .binding(Binding::SystemdCredentials)
            .serve()
            .await?;
        std::future::pending::<()>().await;
        Ok(())
    })
}

fn harden() -> Result<(), Error> {
    set_dumpable_behavior(DumpableBehavior::NotDumpable)
        .map_err(|error| Error::Harden(error.into()))?;
    setrlimit(
        Resource::Core,
        Rlimit {
            current: Some(0),
            maximum: Some(0),
        },
    )
    .map_err(|error| Error::Harden(error.into()))
}

fn folder() -> Result<PathBuf, Error> {
    Ok(rikka_dirs::Paths::resolve()?.data_dir().join(KEYRINGS))
}

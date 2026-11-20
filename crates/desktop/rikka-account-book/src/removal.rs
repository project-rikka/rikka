use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;

use crate::error::{Error, Result};
use crate::vault::Vault;

const THREAD: &str = "rikka-accounts-remove";

#[derive(Debug)]
pub struct Removal(Outcome);

#[derive(Debug)]
enum Outcome {
    Running(JoinHandle<Result<()>>),
    Done(Result<()>),
}

impl Removal {
    pub(crate) fn start(vault: Vault, folder: PathBuf) -> Removal {
        let removing = folder.clone();
        let spawned = std::thread::Builder::new()
            .name(THREAD.into())
            .spawn(move || remove(vault, &removing));
        Removal(match spawned {
            Ok(running) => Outcome::Running(running),
            Err(error) => {
                tracing::warn!(%error, "failed to spawn {THREAD}, removing in place");
                Outcome::Done(remove(vault, &folder))
            }
        })
    }

    pub fn wait(self) -> Result<()> {
        match self.0 {
            Outcome::Running(running) => running
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic)),
            Outcome::Done(outcome) => outcome,
        }
    }
}

fn remove(vault: Vault, folder: &Path) -> Result<()> {
    let removed = match std::fs::remove_dir_all(folder) {
        Err(error) if error.kind() != ErrorKind::NotFound => Err(Error::io(folder)(error)),
        _ => Ok(()),
    };
    let outcome = removed.and(vault.forget(folder));
    if let Err(error) = &outcome {
        tracing::warn!(%error, folder = %folder.display(), "failed to remove account folder");
    }
    outcome
}

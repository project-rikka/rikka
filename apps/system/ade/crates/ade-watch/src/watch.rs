use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use rikka_watch::{Event, Reach, Watcher};

use crate::sink::Sink;

const MAX_DELAY_INTERVALS: u32 = 8;

enum Command {
    Changed(PathBuf),
    Lost,
    SetFolders(BTreeSet<PathBuf>),
    Stop,
}

pub struct Watch {
    commands: Option<Sender<Command>>,
}

impl Watch {
    #[must_use]
    pub fn disabled() -> Watch {
        Watch { commands: None }
    }

    #[must_use]
    pub fn new(quiet: Duration, sink: Sink) -> Watch {
        let (commands, receiver) = mpsc::channel();
        let watcher = match start_watcher(commands.clone()) {
            Ok(watcher) => watcher,
            Err(error) => {
                tracing::warn!(%error, "failed to start file watcher");
                return Watch::disabled();
            }
        };

        let spawned = std::thread::Builder::new()
            .name("rikka-ade-watch".to_owned())
            .spawn(move || run(watcher, &receiver, quiet, &sink));
        match spawned {
            Ok(_) => Watch {
                commands: Some(commands),
            },
            Err(error) => {
                tracing::warn!(%error, "failed to start file watcher");
                Watch::disabled()
            }
        }
    }

    pub fn set_folders(&mut self, folders: BTreeSet<PathBuf>) {
        if let Some(commands) = self.commands.as_ref() {
            let _ = commands.send(Command::SetFolders(folders));
        }
    }
}

fn start_watcher(commands: Sender<Command>) -> rikka_watch::Result<Watcher> {
    Watcher::spawn(move |event| {
        let command = match event {
            Event::Changed(change) => Command::Changed(change.path),
            Event::Lost => Command::Lost,
        };
        let _ = commands.send(command);
    })
}

fn run(watcher: Watcher, receiver: &Receiver<Command>, quiet: Duration, sink: &Sink) {
    let mut watching = BTreeSet::new();
    let mut changed_folders: BTreeSet<PathBuf> = BTreeSet::new();
    let mut deadline: Option<Instant> = None;

    loop {
        let commands = match deadline {
            None => match receiver.recv() {
                Ok(commands) => Some(commands),
                Err(_) => return,
            },
            Some(deadline) => {
                let left = deadline.saturating_duration_since(Instant::now());
                match receiver.recv_timeout(quiet.min(left)) {
                    Ok(commands) => Some(commands),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
        };

        match commands {
            Some(Command::Changed(path)) => {
                if let Some(folder) = folder_of(&path, &watching) {
                    changed_folders.insert(folder);
                    deadline.get_or_insert_with(|| Instant::now() + quiet * MAX_DELAY_INTERVALS);
                }
                continue;
            }
            Some(Command::Lost) => {
                if !watching.is_empty() {
                    changed_folders.extend(watching.iter().cloned());
                    deadline.get_or_insert_with(|| Instant::now() + quiet * MAX_DELAY_INTERVALS);
                }
                continue;
            }
            Some(Command::SetFolders(folders)) => {
                update_watches(&watcher, &mut watching, folders);
                continue;
            }
            Some(Command::Stop) => return,
            None => {}
        }

        deadline = None;
        let folders = std::mem::take(&mut changed_folders);
        if !folders.is_empty() {
            sink(folders);
        }
    }
}

fn update_watches(watcher: &Watcher, watching: &mut BTreeSet<PathBuf>, folders: BTreeSet<PathBuf>) {
    for stale in watching.difference(&folders) {
        watcher.unwatch(stale);
    }
    let added: BTreeSet<PathBuf> = folders
        .difference(watching)
        .filter(|folder| match watcher.watch(folder, Reach::Contents) {
            Ok(()) => true,
            Err(error) => {
                tracing::debug!(%error, path = %folder.display(), "failed to watch folder");
                false
            }
        })
        .cloned()
        .collect();

    watching.retain(|folder| folders.contains(folder));
    watching.extend(added);
}

fn folder_of(path: &Path, watching: &BTreeSet<PathBuf>) -> Option<PathBuf> {
    if watching.contains(path) {
        return Some(path.to_owned());
    }
    let parent = path.parent()?;
    watching.contains(parent).then(|| parent.to_owned())
}

impl Drop for Watch {
    fn drop(&mut self) {
        if let Some(commands) = self.commands.as_ref() {
            let _ = commands.send(Command::Stop);
        }
    }
}

impl std::fmt::Debug for Watch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Watch")
            .field("live", &self.commands.is_some())
            .finish_non_exhaustive()
    }
}

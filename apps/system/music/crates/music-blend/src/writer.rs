use std::convert::Infallible;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::Arc;

use music_core::{Error, Result};
use rikka_worker::{Broadcast, CommandReceiver, CommandSender, Subscription, Worker};

use crate::blend::{Blend, BlendId};
use crate::folder;
use crate::jspf;
use crate::ledger::{Ledger, Placed};

const NAME: &str = "rikka-music-blends";

pub(crate) enum Write {
    Keep(Arc<Blend>),
    Listed(Vec<u8>),
    Gone(BlendId),
}

enum Order {
    Write(Write),
    Finish,
}

#[derive(Clone, Debug)]
pub(crate) struct Places {
    pub(crate) playlists: PathBuf,
    pub(crate) deleted: PathBuf,
}

pub(crate) struct Writer {
    places: Places,
    listeners: Arc<Broadcast<Error>>,
    worker: Worker<CommandSender<Order>>,
}

impl Writer {
    pub(crate) fn start(places: Places, ledger: Ledger) -> Result<Writer> {
        let listeners = Arc::new(Broadcast::new());
        let worker = spawn(places.clone(), ledger, Arc::clone(&listeners))
            .map_err(|error| Error::Spawn(format!("{NAME}: {error}")))?;
        Ok(Writer {
            places,
            listeners,
            worker,
        })
    }

    pub(crate) fn send(&self, write: Write) {
        if let Err(error) = self.worker.handle().send(Order::Write(write)) {
            failed(&self.listeners, stored(error));
        }
    }

    pub(crate) fn failures(&self) -> Subscription<Error> {
        self.listeners.subscribe()
    }
}

impl std::fmt::Debug for Writer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Writer")
            .field("places", &self.places)
            .finish_non_exhaustive()
    }
}

fn spawn(
    places: Places,
    ledger: Ledger,
    listeners: Arc<Broadcast<Error>>,
) -> rikka_worker::Result<Worker<CommandSender<Order>>, Infallible> {
    let (sender, commands) =
        rikka_worker::command_channel(|| Order::Finish).map_err(rikka_worker::Error::Spawn)?;
    Worker::spawn(NAME, sender, move || {
        Ok(move || writing(&commands, &places, &ledger, &listeners))
    })
}

fn writing(
    commands: &CommandReceiver<Order>,
    places: &Places,
    ledger: &Ledger,
    listeners: &Broadcast<Error>,
) {
    loop {
        for order in commands.drain() {
            match order {
                Order::Write(write) => perform(places, ledger, write, listeners),
                Order::Finish => return,
            }
        }
        if let Err(error) = commands.wait() {
            tracing::warn!("blend writer stopped: {error}");
            return;
        }
    }
}

fn perform(places: &Places, ledger: &Ledger, write: Write, listeners: &Broadcast<Error>) {
    if let Err(error) = apply(places, ledger, write) {
        failed(listeners, error);
    }
}

fn failed(listeners: &Broadcast<Error>, error: Error) {
    tracing::warn!("failed to save blend: {error}");
    listeners.send(&error);
}

fn apply(places: &Places, ledger: &Ledger, write: Write) -> Result<()> {
    let mut placements = ledger.lock();
    match write {
        Write::Keep(blend) => {
            fs::create_dir_all(&places.playlists).map_err(stored)?;
            let own = placements.get(&blend.id).map(|placed| placed.path.clone());
            let path = folder::file_for(&places.playlists, &blend.name, own.as_deref());
            let bytes = jspf::write(&blend);
            rikka_fs::replace(&path, &bytes).map_err(stored)?;
            placements.insert(blend.id.clone(), Placed::new(path.clone(), &bytes));
            match own {
                Some(own) if own != path => removed(own),
                _ => Ok(()),
            }
        }
        Write::Listed(listed) => rikka_fs::Replace::new(&places.deleted)
            .private()
            .write(&listed)
            .map_err(stored),
        Write::Gone(id) => match placements.remove(&id) {
            Some(placed) => removed(placed.path),
            None => Ok(()),
        },
    }
}

fn removed(path: PathBuf) -> Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != ErrorKind::NotFound => Err(stored(error)),
        _ => Ok(()),
    }
}

fn stored(error: impl std::fmt::Display) -> Error {
    Error::Storage(error.to_string())
}

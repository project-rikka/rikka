use std::sync::Arc;
use std::thread;

use futures_util::StreamExt;
use rikka_open::path_dirs;
use rikka_sync::Mutex;
use rikka_worker::Observable;

use crate::provider::{ChangeStream, Hit, Query};
use crate::run::catalogue::Catalogue;

const THREAD: &str = "rikka-launcher-path";

#[derive(Debug)]
enum Scan {
    Unchanged,
    Changed(Catalogue),
}

#[derive(Debug, Default)]
pub(crate) struct Shelf {
    catalogue: Catalogue,
    finished: Arc<Mutex<Option<Scan>>>,
    scanned: Observable<()>,
    scanning: bool,
    rescan_requested: bool,
}

impl Shelf {
    pub(crate) fn rescan(&mut self) {
        if self.scanning {
            self.rescan_requested = true;
            return;
        }
        let (finished, scanned) = (Arc::clone(&self.finished), self.scanned.clone());
        let sources = self.catalogue.sources().clone();
        let directories = path_dirs();
        let started = thread::Builder::new()
            .name(THREAD.to_owned())
            .spawn(move || {
                let scan = match Catalogue::reread(&sources, &directories) {
                    Some(catalogue) => Scan::Changed(catalogue),
                    None => Scan::Unchanged,
                };
                *finished.lock() = Some(scan);
                scanned.set(());
            });
        match started {
            Ok(_) => self.scanning = true,
            Err(error) => tracing::warn!("cannot spawn `$PATH` reader thread: {error}"),
        }
    }

    pub(super) fn take_scan(&mut self) {
        let finished = self.finished.lock().take();
        let Some(scan) = finished else {
            return;
        };
        if let Scan::Changed(catalogue) = scan {
            self.catalogue = catalogue;
        }
        self.scanning = false;
        if std::mem::take(&mut self.rescan_requested) {
            self.rescan();
        }
    }

    pub(crate) fn changes(&self) -> ChangeStream {
        self.scanned.changes().boxed()
    }

    pub(super) fn knows(&self, program: &str) -> bool {
        self.catalogue.knows(program)
    }

    pub(crate) fn search(&self, query: &Query, limit: usize) -> Vec<Hit> {
        self.catalogue.search(query, limit)
    }
}

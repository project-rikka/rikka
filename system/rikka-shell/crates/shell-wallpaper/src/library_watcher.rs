use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, OnceLock};
use std::thread;

use rikka_seam::Responder;
use rikka_xdg::Watch;

use crate::shelf::Shelf;

#[derive(Clone, Debug)]
pub(crate) struct LibraryWatcher(Arc<Inner>);

#[derive(Debug)]
struct Inner {
    jobs: Sender<Job>,
    announcing: Arc<AtomicBool>,
    watch: OnceLock<Watch>,
}

#[derive(Debug)]
enum Job {
    List(Responder),
    Announce,
}

impl LibraryWatcher {
    #[must_use]
    pub(crate) fn new(shelf: Shelf) -> LibraryWatcher {
        let (jobs, requests) = channel::<Job>();
        let announcing = Arc::new(AtomicBool::new(false));
        let library_watcher = LibraryWatcher(Arc::new(Inner {
            jobs,
            announcing: Arc::clone(&announcing),
            watch: OnceLock::new(),
        }));
        if let Some(folder) = shelf.folder() {
            library_watcher.watching(folder);
        }
        let spawned = thread::Builder::new()
            .name("rikka-wallpaper-library".to_owned())
            .spawn(move || {
                for job in requests {
                    match job {
                        Job::List(responder) => match shelf.listing() {
                            Ok(listed) => responder.encode(&listed),
                            Err(error) => responder.fail(error.into()),
                        },
                        Job::Announce => {
                            announcing.store(false, Ordering::Release);
                            shelf.announce();
                        }
                    }
                }
            });
        if let Err(error) = spawned {
            tracing::error!("failed to read wallpaper library: {error}");
        }
        library_watcher
    }

    pub(crate) fn list(&self, responder: Responder) {
        self.ask(Job::List(responder));
    }

    pub(crate) fn announce(&self) {
        if self.0.announcing.swap(true, Ordering::AcqRel) {
            return;
        }
        self.ask(Job::Announce);
    }

    fn watching(&self, folder: &Path) {
        let asking = Arc::downgrade(&self.0);
        let watch = Watch::folder(folder, move || {
            if let Some(inner) = asking.upgrade() {
                LibraryWatcher(inner).announce();
            }
        });
        match watch {
            Ok(watch) => {
                if self.0.watch.set(watch).is_err() {
                    tracing::debug!("wallpaper folder already watched");
                }
            }
            Err(error) => tracing::warn!(
                "cannot watch {}, new wallpapers appear on next rescan: {error}",
                folder.display()
            ),
        }
    }

    fn ask(&self, job: Job) {
        if self.0.jobs.send(job).is_err() {
            tracing::warn!("wallpaper library reader stopped");
        }
    }
}

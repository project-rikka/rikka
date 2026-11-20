use std::sync::{Arc, OnceLock};

use futures_util::{Stream, StreamExt};
use rikka_sync::{Mutex, RwLock};
use rikka_worker::Observable;
use rikka_xdg::{Catalog, Watch};
use shell_kit::detached;

const SCANNING: &str = "rikka-shell-applications";

#[derive(Clone, Debug)]
pub struct Installed(Arc<Inner>);

impl Installed {
    #[must_use]
    pub(super) fn watched() -> Installed {
        let installed = Installed(Arc::new(Inner {
            catalogue: RwLock::new(Arc::new(Catalog::default())),
            scans: Observable::new(0),
            listeners: Mutex::new(Vec::new()),
            watch: OnceLock::new(),
        }));
        let weak = Arc::downgrade(&installed.0);
        let watch = Watch::applications(move || {
            if let Some(weak) = weak.upgrade() {
                weak.rescan();
            }
        });
        match watch {
            Ok(watch) => {
                if installed.0.watch.set(watch).is_err() {
                    tracing::debug!("installed applications already watched");
                }
            }
            Err(error) => tracing::warn!(
                "cannot watch installed applications, rescanning only when the launcher opens: \
                 {error}"
            ),
        }
        installed.rescan_in_background();
        installed
    }

    pub fn rescan_if_stale(&self) {
        if self.0.watch.get().is_none() && self.0.catalogue().is_stale() {
            self.rescan_in_background();
        }
    }

    fn rescan_in_background(&self) {
        let weak = Arc::downgrade(&self.0);
        detached(SCANNING, move || {
            if let Some(inner) = weak.upgrade() {
                inner.rescan();
            }
        });
    }

    #[must_use]
    pub(crate) fn scan(&self) -> u64 {
        self.0.scans.get()
    }

    pub fn on_change(&self, event: impl Fn(&Catalog) + Send + Sync + 'static) {
        self.0.listeners.lock().push(Arc::new(event));
    }

    pub fn changes(&self) -> impl Stream<Item = ()> + Send + 'static {
        self.0.scans.changes().map(|_| ())
    }

    #[must_use]
    pub fn with<R>(&self, read: impl FnOnce(&Catalog) -> R) -> R {
        read(&self.0.catalogue())
    }

    #[must_use]
    pub fn current(&self) -> Arc<Catalog> {
        self.0.catalogue()
    }
}

type Listener = Arc<dyn Fn(&Catalog) + Send + Sync>;

struct Inner {
    catalogue: RwLock<Arc<Catalog>>,
    scans: Observable<u64>,
    listeners: Mutex<Vec<Listener>>,
    watch: OnceLock<Watch>,
}

impl Inner {
    fn catalogue(&self) -> Arc<Catalog> {
        Arc::clone(&self.catalogue.read())
    }

    fn reread(&self) -> Arc<Catalog> {
        let fresh = Arc::new(Catalog::read());
        *self.catalogue.write() = Arc::clone(&fresh);
        self.scans.modify(|reading| {
            *reading += 1;
            true
        });
        fresh
    }

    fn rescan(&self) {
        let fresh = self.reread();
        let listeners = self.listeners.lock().clone();
        for event in listeners {
            event(&fresh);
        }
    }
}

impl std::fmt::Debug for Inner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Inner")
            .field("scan", &self.scans.get())
            .finish_non_exhaustive()
    }
}

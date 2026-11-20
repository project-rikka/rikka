use rikka_sync::Mutex;
use std::sync::Arc;

use files_open::Applications;
use rikka_xdg::{MimeApps, Watch};

type Loaded = Arc<Mutex<Option<Arc<Applications>>>>;

#[derive(Debug)]
pub(crate) struct Programs {
    loaded: Loaded,
    _watch: Option<Watch>,
}

impl Programs {
    pub(crate) fn start() -> Self {
        let loaded: Loaded = Arc::default();
        let preloading = Arc::clone(&loaded);
        let started = std::thread::Builder::new()
            .name("rikka-files-applications".to_owned())
            .spawn(move || {
                let mut preloaded = preloading.lock();
                if preloaded.is_none() {
                    *preloaded = Some(Arc::new(Applications::load()));
                }
            });
        if let Err(error) = started {
            tracing::warn!(%error, "failed to preload applications");
        }
        let watch = MimeApps::session().and_then(|apps| {
            apps.watch(reloading(&loaded))
                .inspect_err(
                    |error| tracing::warn!(%error, "cannot watch application associations"),
                )
                .ok()
        });
        Self {
            loaded,
            _watch: watch,
        }
    }

    pub(crate) fn applications(&self) -> Arc<Applications> {
        let mut loaded = self.loaded.lock();
        Arc::clone(loaded.get_or_insert_with(|| Arc::new(Applications::load())))
    }
}

fn reloading(loaded: &Loaded) -> impl Fn() + Send + 'static {
    let loaded = Arc::clone(loaded);
    move || {
        let fresh = Arc::new(Applications::load());
        *loaded.lock() = Some(fresh);
    }
}

use std::cell::OnceCell;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use browser_store::Store;

use browser_model::{Geometry, Loaded};

const DATABASE: &str = "browser/browser.sqlite";

pub(crate) struct Startup {
    reading: Option<JoinHandle<(Store, Loaded)>>,
    shaped: Receiver<Option<Geometry>>,
    geometry: OnceCell<Option<Geometry>>,
}

impl Startup {
    pub(crate) fn begin() -> Startup {
        let (shape, shaped) = mpsc::channel();
        let reading = thread::Builder::new()
            .name("rikka-browser-load".to_owned())
            .spawn(move || read(&shape));
        let reading = reading
            .inspect_err(|error| tracing::error!(%error, "failed to open browser store"))
            .ok();
        Startup {
            reading,
            shaped,
            geometry: OnceCell::new(),
        }
    }

    pub(crate) fn first_geometry(&self) -> Option<Geometry> {
        *self
            .geometry
            .get_or_init(|| self.shaped.recv().ok().flatten())
    }

    pub(crate) fn finish(self) -> (Store, Loaded) {
        match self.reading.map(JoinHandle::join) {
            Some(Ok(read)) => read,
            Some(Err(_)) | None => (memory(), Loaded::default()),
        }
    }
}

fn read(shape: &Sender<Option<Geometry>>) -> (Store, Loaded) {
    let store = match path().map(Store::open) {
        Some(Ok(store)) => store,
        Some(Err(error)) => {
            tracing::error!(%error, "browser store unavailable, using memory");
            memory()
        }
        None => {
            tracing::error!("no data directory for the browser store");
            memory()
        }
    };
    let _ = shape.send(first_geometry(&store));
    let loaded = match Loaded::read(&store) {
        Ok(loaded) => loaded,
        Err(error) => {
            tracing::error!(%error, "failed to open browser store");
            Loaded::default()
        }
    };
    (store, loaded)
}

fn first_geometry(store: &Store) -> Option<Geometry> {
    let session = store.load_session().ok()?;
    let window = session.windows.first()?;
    let bounds = window.bounds?;
    Some(Geometry {
        width: bounds.width,
        height: bounds.height,
        maximized: window.maximized,
    })
}

fn path() -> Option<PathBuf> {
    let paths = rikka_dirs::Paths::resolve().ok()?;
    Some(paths.data_dir().join(DATABASE))
}

fn memory() -> Store {
    Store::in_memory().expect("in-memory store opens")
}

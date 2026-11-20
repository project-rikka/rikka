use rikka_sync::Mutex;
use std::sync::Arc;

use rikka_proton_api::offload;
use rikka_proton_drive_sync_state::Store;

use crate::error::Result;

#[derive(Debug, Clone)]
pub(crate) struct Persist {
    store: Arc<Mutex<Store>>,
}

impl Persist {
    pub(crate) fn new(store: Store) -> Persist {
        Persist {
            store: Arc::new(Mutex::new(store)),
        }
    }

    pub(crate) async fn run<T, W>(&self, work: W) -> Result<T>
    where
        T: Send + 'static,
        W: FnOnce(&mut Store) -> rikka_proton_drive_sync_state::Result<T> + Send + 'static,
    {
        let store = Arc::clone(&self.store);
        let result = offload(move || {
            let mut guard = store.lock();
            work(&mut guard)
        })
        .await;
        Ok(result?)
    }
}

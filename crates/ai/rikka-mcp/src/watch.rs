use rikka_sync::Mutex;
use std::sync::Arc;

#[derive(Clone, Default)]
pub(crate) struct Watch {
    callback: Arc<Mutex<Option<Callback>>>,
}

type Callback = Arc<dyn Fn(&str) + Send + Sync>;

impl Watch {
    pub(crate) fn set(&self, callback: impl Fn(&str) + Send + Sync + 'static) {
        *self.callback.lock() = Some(Arc::new(callback));
    }

    pub(crate) fn say(&self, server: &str) {
        let callback = self.callback.lock().clone();
        if let Some(callback) = callback {
            callback(server);
        }
    }
}

impl std::fmt::Debug for Watch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Watch")
            .field("watched", &self.callback.lock().is_some())
            .finish_non_exhaustive()
    }
}

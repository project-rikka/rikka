use rikka_sync::Mutex;
use rikka_worker::EventNotifier;

#[derive(Default)]
pub(crate) struct Hotplug {
    generation: Mutex<u64>,
    watchers: Mutex<Vec<EventNotifier>>,
}

impl Hotplug {
    pub(crate) fn notify(&self) {
        *self.generation.lock() += 1;
        for watcher in self.watchers.lock().iter() {
            if let Err(error) = watcher.notify() {
                tracing::warn!(%error, "failed to announce an audio device change");
            }
        }
    }

    pub(crate) fn watch(&self, bell: EventNotifier) {
        self.watchers.lock().push(bell);
    }

    pub(crate) fn generation(&self) -> u64 {
        *self.generation.lock()
    }
}

impl std::fmt::Debug for Hotplug {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Hotplug")
            .field("generation", &self.generation())
            .finish_non_exhaustive()
    }
}

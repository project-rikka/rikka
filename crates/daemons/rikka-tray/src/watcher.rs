use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::id::ItemId;
use crate::registry::Registry;
use crate::session::{Changes, Trays};
use rikka_bus::Notifier;

const PROTOCOL: i32 = 0;

pub(crate) struct Watcher {
    registry: Arc<Registry>,
    notifier: Notifier<Trays>,
    hosted: AtomicBool,
}

impl Watcher {
    pub(crate) fn new(registry: Arc<Registry>, notifier: Notifier<Trays>) -> Watcher {
        Watcher {
            registry,
            notifier,
            hosted: AtomicBool::new(false),
        }
    }
}

#[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
impl Watcher {
    fn register_status_notifier_item(
        &self,
        service: String,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) {
        let Some(sender) = header.sender() else {
            return tracing::debug!("tray item registered without a sender: {service}");
        };
        let item = ItemId::registered(&service, sender.as_str());
        if self.registry.add(item.clone()) {
            self.notifier.notify(Changes::registered(item));
        }
    }

    fn register_status_notifier_host(&self, service: String) {
        self.hosted.store(true, Ordering::Relaxed);
        self.notifier.notify(Changes::hosted(service));
    }

    #[zbus(property)]
    fn registered_status_notifier_items(&self) -> Vec<String> {
        self.registry
            .items()
            .iter()
            .map(ItemId::to_string)
            .collect()
    }

    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        self.hosted.load(Ordering::Relaxed)
    }

    #[zbus(property)]
    fn protocol_version(&self) -> i32 {
        PROTOCOL
    }
}

impl std::fmt::Debug for Watcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Watcher").finish_non_exhaustive()
    }
}

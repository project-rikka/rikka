use std::collections::HashMap;

use zbus::zvariant::{OwnedValue, Type, Value};

pub(crate) const WATCHER: &str = "org.kde.StatusNotifierWatcher";
pub(crate) const WATCHER_PATH: &str = "/StatusNotifierWatcher";

pub(crate) const ITEM: &str = "org.kde.StatusNotifierItem";
pub(crate) const ITEM_ALTERNATIVE: &str = "org.freedesktop.StatusNotifierItem";

pub(crate) const MENU: &str = "com.canonical.dbusmenu";

pub(crate) const HOST_PREFIX: &str = "org.kde.StatusNotifierHost-";

pub(crate) const ITEM_SIGNALS: [&str; 6] = [
    "NewTitle",
    "NewIcon",
    "NewAttentionIcon",
    "NewOverlayIcon",
    "NewToolTip",
    "NewStatus",
];

#[derive(Debug, Type, Value, OwnedValue, serde::Deserialize)]
pub(crate) struct RawLayout {
    pub(crate) id: i32,
    pub(crate) properties: HashMap<String, OwnedValue>,
    pub(crate) children: Vec<OwnedValue>,
}

#[zbus::proxy(
    interface = "org.kde.StatusNotifierWatcher",
    default_service = "org.kde.StatusNotifierWatcher",
    default_path = "/StatusNotifierWatcher",
    gen_async = false
)]
pub(crate) trait Watcher {
    fn register_status_notifier_item(&self, service: &str) -> zbus::Result<()>;

    fn register_status_notifier_host(&self, service: &str) -> zbus::Result<()>;

    #[zbus(property)]
    fn registered_status_notifier_items(&self) -> zbus::Result<Vec<String>>;
}

#[zbus::proxy(
    interface = "org.kde.StatusNotifierItem",
    assume_defaults = false,
    gen_async = false
)]
pub(crate) trait Item {
    #[zbus(no_reply)]
    fn activate(&self, x: i32, y: i32) -> zbus::Result<()>;

    #[zbus(no_reply)]
    fn secondary_activate(&self, x: i32, y: i32) -> zbus::Result<()>;

    #[zbus(no_reply)]
    fn context_menu(&self, x: i32, y: i32) -> zbus::Result<()>;

    #[zbus(no_reply)]
    fn scroll(&self, delta: i32, orientation: &str) -> zbus::Result<()>;
}

#[zbus::proxy(
    interface = "com.canonical.dbusmenu",
    assume_defaults = false,
    gen_async = false
)]
pub(crate) trait Menu {
    fn get_layout(
        &self,
        parent_id: i32,
        recursion_depth: i32,
        property_names: &[&str],
    ) -> zbus::Result<(u32, RawLayout)>;

    fn about_to_show(&self, id: i32) -> zbus::Result<bool>;

    #[zbus(no_reply)]
    fn event(&self, id: i32, event_id: &str, data: &Value<'_>, timestamp: u32) -> zbus::Result<()>;
}

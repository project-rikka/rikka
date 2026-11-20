use std::collections::HashMap;
use std::sync::Arc;

use zbus::zvariant::OwnedValue;

use crate::inbox::Inbox;
use crate::notification::{Notification, Posting};

pub(crate) const NAME: &str = "org.freedesktop.Notifications";
pub(crate) const PATH: &str = "/org/freedesktop/Notifications";

const CAPABILITIES: [&str; 7] = [
    "action-icons",
    "actions",
    "body",
    "body-hyperlinks",
    "body-markup",
    "icon-static",
    "persistence",
];

const PRODUCT: &str = "Rikka";
const VENDOR: &str = "Rikka";
const SPECIFICATION: &str = "1.2";

pub(crate) struct Server {
    inbox: Arc<Inbox>,
}

impl Server {
    #[must_use]
    pub(crate) fn new(inbox: Arc<Inbox>) -> Server {
        Server { inbox }
    }
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Server {
    fn get_capabilities(&self) -> Vec<&'static str> {
        CAPABILITIES.to_vec()
    }

    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
    ) -> u32 {
        let posting = Posting {
            app_name,
            app_icon,
            summary,
            body,
            actions,
            hints,
            expire_timeout,
        };
        self.inbox.post(replaces_id, Notification::read(posting))
    }

    fn close_notification(&self, id: u32) {
        self.inbox.withdraw(id);
    }

    fn get_server_information(&self) -> (&'static str, &'static str, &'static str, &'static str) {
        (PRODUCT, VENDOR, env!("CARGO_PKG_VERSION"), SPECIFICATION)
    }
}

impl std::fmt::Debug for Server {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Server").finish_non_exhaustive()
    }
}

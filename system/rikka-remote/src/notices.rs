use std::sync::Arc;

use rikka_i18n::tr;
use rikka_notify::{Icon, Notification, Notifications, Urgency};

use crate::uploads::Received;

const APP_NAME: &str = "Rikka";
const REMOTE_ICON: &str = "preferences-desktop-remote-desktop";
const FILE_ICON: &str = "document-save";

#[derive(Clone, Debug)]
pub(crate) struct Notices {
    notifications: Arc<Notifications>,
    runtime: tokio::runtime::Handle,
}

impl Notices {
    pub(crate) fn new(runtime: tokio::runtime::Handle) -> Notices {
        Notices {
            notifications: Arc::new(Notifications::new()),
            runtime,
        }
    }

    pub(crate) fn connected(&self, who: &str) {
        self.show(
            Notification::new(tr!("remote.connected.summary"))
                .body(tr!("remote.connected.body", who = who))
                .icon(Icon::named(REMOTE_ICON))
                .urgency(Urgency::Normal),
        );
    }

    pub(crate) fn refused(&self, from: &str) {
        self.show(
            Notification::new(tr!("remote.refused.summary"))
                .body(tr!("remote.refused.body", from = from))
                .icon(Icon::named(REMOTE_ICON))
                .urgency(Urgency::Critical),
        );
    }

    pub(crate) fn received(&self, received: &Received) {
        let folder = received.folder.display().to_string();
        self.show(
            Notification::new(tr!("remote.received.summary"))
                .body(tr!(
                    "remote.received.body",
                    name = received.name.as_str(),
                    folder = folder.as_str()
                ))
                .icon(Icon::named(FILE_ICON)),
        );
    }

    fn show(&self, notification: Notification) {
        let notifications = Arc::clone(&self.notifications);
        let notification = notification.app_name(APP_NAME);
        self.runtime.spawn(async move {
            if let Err(error) = notifications.notify(&notification).await {
                tracing::warn!("cannot show a notification: {error}");
            }
        });
    }
}

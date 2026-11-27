use rikka_app::prelude::*;
use rikka_notify::{Invoked, Ledger, Notifications};
use rikka_web::DownloadId;

use crate::shell::Shell;
use browser_model::{ProfileKey, WindowId};

pub(crate) const ENTRY: &str = "rikka-browser";

#[derive(Debug, Default)]
pub(crate) struct Alerts {
    notifications: Notifications,
    shown: Ledger<ActiveNotification>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ActiveNotification {
    pub(crate) profile: ProfileKey,
    pub(crate) download: DownloadId,
    pub(crate) window: WindowId,
}

impl Alerts {
    pub(crate) fn notifications(&self) -> &Notifications {
        &self.notifications
    }
}

impl Shell {
    pub(crate) fn post(&mut self, desktop: rikka_notify::Notification, shown: ActiveNotification) {
        let notifications = self
            .browser()
            .page_services()
            .alerts_mut()
            .notifications()
            .clone();
        spawn(
            async move { notifications.notify(&desktop).await },
            move |shell: &mut Shell, sent: rikka_notify::Result<rikka_notify::Id>| match sent {
                Ok(id) => shell
                    .browser()
                    .page_services()
                    .alerts_mut()
                    .shown
                    .record(id, shown),
                Err(error) => tracing::warn!(%error, "failed to show notification"),
            },
        );
    }

    pub(crate) fn alert_event(&mut self, event: rikka_notify::Event) {
        let invoked = self
            .browser()
            .page_services()
            .alerts_mut()
            .shown
            .settle(event);
        let Some(Invoked { shown, key, token }) = invoked else {
            return;
        };
        self.download_notice_answered(shown, &key, token);
    }
}

pub(crate) fn activate(token: Option<String>) {
    if let Some(token) = token {
        request_window(WindowRequest::Activate(token));
    }
}

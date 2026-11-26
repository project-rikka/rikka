use discord_core::{ChannelId, Notice};
use discord_preferences::Alerts;
use rikka_app::prelude::tr;
use rikka_notify::{Icon, Invoked, Ledger, Notifications};

pub(crate) const ENTRY: &str = "rikka-discord";
const BODY: usize = 240;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NotificationTarget {
    pub(crate) key: String,
    pub(crate) channel: ChannelId,
}

#[derive(Debug, Default)]
pub(crate) struct Notices {
    notifications: Notifications,
    shown: Ledger<NotificationTarget>,
}

impl Notices {
    pub(crate) fn notifications(&self) -> &Notifications {
        &self.notifications
    }

    pub(crate) fn shown(&mut self, id: rikka_notify::Id, opens: NotificationTarget) {
        self.shown.record(id, opens);
    }

    pub(crate) fn settle(
        &mut self,
        event: rikka_notify::Event,
    ) -> Option<Invoked<NotificationTarget>> {
        self.shown.settle(event)
    }
}

pub(crate) fn worth(alerts: Alerts, notice: &Notice, looking: bool) -> bool {
    if !alerts.enabled || looking {
        return false;
    }
    match notice.space {
        None => alerts.directs,
        Some(_) if notice.message.mentions_me => alerts.mentions,
        Some(_) => true,
    }
}

pub(crate) fn desktop(title: String, body: String, open: String) -> rikka_notify::Notification {
    let mut body: String = body.chars().take(BODY).collect();
    if body.is_empty() {
        body = String::from("…");
    }
    rikka_notify::Notification::new(title)
        .body(body)
        .app_name(tr!("discord.title").to_string())
        .desktop_entry(ENTRY)
        .icon(Icon::named(ENTRY))
        .category("im.received")
        .default_action(open)
}

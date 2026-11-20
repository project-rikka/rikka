use discord_core::Notice;
use rikka_app::prelude::*;
use rikka_notify::Invoked;

use super::Chat;
use crate::notices::{NotificationTarget, desktop, worth};

const EVENTS: &str = "discord-notice-events";

impl Chat {
    pub(crate) fn listen_for_notices(&mut self) {
        let events = self.notices.notifications().events();
        stream_keyed(
            EVENTS,
            events,
            |chat: &mut Chat, event: rikka_notify::Event| {
                chat.notice_event(event);
            },
        );
    }

    pub(crate) fn focused(&mut self, focused: bool) {
        self.focused = focused;
        if !focused {
            self.push_to_talk(false);
        }
        skip_view();
    }

    pub(crate) fn notice(&mut self, key: &str, notice: Notice) {
        let Some(linked) = self.linked(key) else {
            return;
        };
        let channel = notice.message.channel;
        let looking = self.focused
            && self.current_key() == Some(key)
            && linked.location().channel.as_ref() == Some(&channel);
        if !worth(self.preferences.alerts(), &notice, looking) {
            return;
        }
        let space = notice.space.as_ref();
        let author = linked.display_name(space, &notice.message.author);
        let title = match linked.channel(&channel) {
            Some(channel_info) if channel_info.space.is_some() => {
                let place = space
                    .and_then(|space| linked.space(space))
                    .map(|space| space.name.clone())
                    .unwrap_or_default();
                format!("{author} (#{}, {place})", channel_info.name)
            }
            Some(channel_info) if !channel_info.name.is_empty() => {
                format!("{author} ({})", channel_info.name)
            }
            _ => author,
        };
        let body = match notice.message.content.is_empty() {
            true => notice.message.text.clone(),
            false => notice.message.content.plain_text(),
        };
        let shown = desktop(title, body, tr!("discord.notice.open").to_string());
        let notifications = self.notices.notifications().clone();
        let opens = NotificationTarget {
            key: key.to_owned(),
            channel,
        };
        spawn(
            async move { notifications.notify(&shown).await },
            move |chat: &mut Chat, sent: rikka_notify::Result<rikka_notify::Id>| {
                skip_view();
                match sent {
                    Ok(id) => chat.notices.shown(id, opens.clone()),
                    Err(error) => tracing::warn!("failed to show notification: {error}"),
                }
            },
        );
    }

    fn notice_event(&mut self, event: rikka_notify::Event) {
        let Some(Invoked {
            shown: opens,
            token,
            ..
        }) = self.notices.settle(event)
        else {
            skip_view();
            return;
        };
        if let Some(token) = token {
            request_window(WindowRequest::Activate(token));
        }
        self.settings.close();
        if self.current_key() != Some(opens.key.as_str()) {
            self.switch(&opens.key);
        }
        self.open_channel(&opens.channel);
    }
}

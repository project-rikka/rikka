use std::ops::Range;
use std::time::Duration;

use discord_core::{ChannelId, Delivery, MessageId, Unread, Update};
use rikka_app::prelude::*;

use super::items::Since;
use crate::chat::Chat;

const READ_AFTER: Duration = Duration::from_millis(800);
const FLASH_FOR: Duration = Duration::from_secs(2);
const READING: &str = "discord-read";
const FLASHING: &str = "discord-flash";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum JumpTarget {
    Message(MessageId),
    Present,
}

#[derive(State)]
pub(crate) struct Reading {
    #[untracked]
    jump: Option<JumpTarget>,
    #[untracked]
    keep_unread_divider: bool,
    #[untracked]
    place: Place,
    since: Tracked<Option<Since>>,
    left_at: Tracked<Option<MessageId>>,
    reveal: Tracked<Option<MessageId>>,
    flash: Tracked<Option<MessageId>>,
}

impl Reading {
    pub(crate) fn new() -> Reading {
        Reading {
            jump: None,
            keep_unread_divider: false,
            place: Place::new(),
            since: Tracked::new(None),
            left_at: Tracked::new(None),
            reveal: Tracked::new(None),
            flash: Tracked::new(None),
        }
    }

    pub(crate) fn since(&self) -> Option<&Since> {
        self.since.as_ref()
    }

    pub(crate) fn place(&self) -> &Place {
        &self.place
    }

    pub(crate) fn left_at(&self) -> Option<&MessageId> {
        self.left_at.as_ref()
    }

    pub(crate) fn revealing(&self) -> Option<&MessageId> {
        self.reveal.as_ref()
    }

    pub(crate) fn flashed(&self) -> Option<&MessageId> {
        self.flash.as_ref()
    }

    pub(crate) fn read_own(&mut self) {
        self.since.set(None);
        self.keep_unread_divider = false;
    }

    pub(crate) fn reset(&mut self, since: Option<Since>) {
        self.jump = None;
        self.keep_unread_divider = false;
        self.since.set(since);
        self.left_at.set(None);
        self.reveal.set(None);
        self.flash.set(None);
    }

    pub(crate) fn request_jump(&mut self, jump: JumpTarget) {
        self.jump = Some(jump);
    }

    pub(crate) fn take_jump(&mut self) -> Option<JumpTarget> {
        self.jump.take()
    }

    pub(crate) fn reveal(&mut self, message: MessageId, flash: bool) {
        self.reveal.set(Some(message));
        if flash {
            self.flash.set(Some(message));
            after_keyed(FLASHING, FLASH_FOR, |reading: &mut Reading| {
                reading.flash.set(None);
            });
        }
    }

    fn revealed(&mut self) {
        if self.reveal.is_some() {
            self.reveal.set(None);
        }
    }
}

impl std::fmt::Debug for Reading {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Reading")
            .field("since", &*self.since)
            .field("left_at", &*self.left_at)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Viewport {
    pub(crate) key: String,
    pub(crate) channel: ChannelId,
    pub(crate) count: usize,
    pub(crate) revealing: Option<usize>,
    pub(crate) last: Option<MessageId>,
    pub(crate) oldest: bool,
    pub(crate) newest: bool,
}

impl Chat {
    pub(crate) fn visible_rows(&mut self, viewport: &Viewport, shown: Range<usize>) {
        let location = self
            .conversation()
            .shown()
            .is_some_and(|(key, channel)| *key == viewport.key && *channel == viewport.channel);
        if !location {
            return;
        }
        let at_end = shown.end >= viewport.count;
        let reading = &mut self.conversation_mut().reading;
        match at_end {
            true => reading.left_at.set(None),
            false => {
                if reading.left_at.is_none() && viewport.newest {
                    reading.left_at.set(viewport.last);
                }
            }
        }
        if viewport.revealing.is_some_and(|at| shown.contains(&at)) {
            reading.revealed();
        }
        if shown.start == 0 && !viewport.oldest {
            self.load_older(&viewport.channel);
        }
        if at_end && !viewport.newest {
            self.load_newer(&viewport.channel);
        }
        if at_end && viewport.newest {
            after_keyed(READING, READ_AFTER, |chat: &mut Chat| chat.read_shown());
        }
    }

    pub(crate) fn read_shown(&mut self) {
        let Some((key, channel)) = self.conversation().shown().cloned() else {
            return;
        };
        if self.conversation().reading.keep_unread_divider {
            return;
        }
        let Some(linked) = self.linked_mut(&key) else {
            return;
        };
        let Some(last) = linked
            .timeline(&channel)
            .and_then(|timeline| timeline.last().cloned())
        else {
            return;
        };
        let unseen = linked.unseen(&channel);
        let read = linked.last_read(&channel) == Some(&last);
        if read && !unseen.unread && unseen.mentions == 0 {
            return;
        }
        linked.apply(Update::Unread(Unread {
            channel,
            unread: false,
            mentions: 0,
            last_read: Some(last),
        }));
        let client = linked.client().clone();
        spawn(
            async move { client.mark_read(&channel, &last).await },
            |marked: discord_core::Result<()>| {
                if let Err(error) = marked {
                    tracing::debug!("failed to mark channel read: {error}");
                }
                skip_view();
            },
        );
    }

    pub(crate) fn mark_unread(&mut self, message: &MessageId) {
        let Some((key, channel)) = self.conversation().shown().cloned() else {
            return;
        };
        let Some(linked) = self.linked_mut(&key) else {
            return;
        };
        let Some(timeline) = linked.timeline(&channel) else {
            return;
        };
        let delivered: Vec<&MessageId> = timeline
            .messages()
            .iter()
            .filter(|known| known.delivery == Delivery::Delivered)
            .map(|known| &known.id)
            .collect();
        let Some(at) = delivered.iter().position(|known| *known == message) else {
            return;
        };
        let before = at.checked_sub(1).map(|at| *delivered[at]);
        linked.apply(Update::Unread(Unread {
            channel,
            unread: true,
            mentions: 0,
            last_read: before,
        }));
        let client = linked.client().clone();
        let reading = &mut self.conversation_mut().reading;
        reading.keep_unread_divider = true;
        reading.since.set(Some(match &before {
            Some(read) => Since::After(*read),
            None => Since::Start,
        }));
        cancel(READING);
        if let Some(before) = before {
            spawn(
                async move { client.mark_read(&channel, &before).await },
                |_: discord_core::Result<()>| skip_view(),
            );
        }
    }
}

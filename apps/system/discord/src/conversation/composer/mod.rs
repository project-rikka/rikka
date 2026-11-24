mod complete;
mod handling;
mod typing;
pub(crate) mod uploads;
mod view;

use std::collections::HashMap;
use std::time::Instant;

use discord_core::{Emoji, MessageId};
use rikka_app::prelude::*;

use crate::conversation::Seat;

pub(crate) use complete::{Offers, Picked};
use uploads::Pending;
pub(crate) use view::{EMOJI_BUTTON, preview, sendable, view};

pub(crate) fn write_emoji(draft: &TextDocument, picked: &mut Vec<(String, Picked)>, emoji: &Emoji) {
    let display = match emoji.is_custom() {
        true => format!(":{}:", emoji.name),
        false => emoji.name.clone(),
    };
    draft.insert(&display);
    if emoji.is_custom() {
        remember(picked, display, Picked::Emoji(emoji.clone()));
    }
}

fn remember(picked: &mut Vec<(String, Picked)>, display: String, value: Picked) {
    picked.retain(|(known, _)| *known != display);
    picked.push((display, value));
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Replying {
    pub(crate) message: MessageId,
    pub(crate) author: String,
    pub(crate) mention: bool,
}

#[derive(Default)]
struct SeatDraft {
    text: String,
    reply: Option<Replying>,
    uploads: Vec<Pending>,
    picked: Vec<(String, Picked)>,
}

#[derive(State)]
pub(crate) struct Composer {
    #[untracked]
    draft: TextDocument,
    #[untracked]
    edit_draft: TextDocument,
    #[untracked]
    drafts: HashMap<Seat, SeatDraft>,
    #[untracked]
    picked: Vec<(String, Picked)>,
    #[untracked]
    descriptions: HashMap<u64, TextDocument>,
    #[untracked]
    typed_at: Option<Instant>,
    #[untracked]
    pub(super) sent_at: HashMap<Seat, Instant>,
    #[untracked]
    next_upload: u64,
    reply: Tracked<Option<Replying>>,
    editing: Tracked<Option<MessageId>>,
    uploads: Tracked<Vec<Pending>>,
    offers: Tracked<Option<Offers>>,
    pub(super) refused: Tracked<Option<String>>,
    pub(super) cooldown: Tracked<u32>,
}

impl Composer {
    pub(crate) fn new() -> Composer {
        Composer {
            draft: TextDocument::new(""),
            edit_draft: TextDocument::new(""),
            drafts: HashMap::new(),
            picked: Vec::new(),
            descriptions: HashMap::new(),
            typed_at: None,
            sent_at: HashMap::new(),
            next_upload: 0,
            reply: Tracked::new(None),
            editing: Tracked::new(None),
            uploads: Tracked::new(Vec::new()),
            offers: Tracked::new(None),
            refused: Tracked::new(None),
            cooldown: Tracked::new(0),
        }
    }

    pub(crate) fn draft(&self) -> &TextDocument {
        &self.draft
    }

    pub(crate) fn edit_draft(&self) -> &TextDocument {
        &self.edit_draft
    }

    pub(crate) fn reply(&self) -> Option<&Replying> {
        self.reply.as_ref()
    }

    pub(crate) fn editing(&self) -> Option<&MessageId> {
        self.editing.as_ref()
    }

    pub(crate) fn uploads(&self) -> &[Pending] {
        &self.uploads
    }

    pub(crate) fn offers(&self) -> Option<&Offers> {
        self.offers.as_ref()
    }

    pub(crate) fn refused(&self) -> Option<&str> {
        self.refused.as_deref()
    }

    pub(crate) fn cooldown(&self) -> u32 {
        *self.cooldown
    }

    pub(crate) fn cancel_reply(&mut self) {
        self.reply.set(None);
    }

    pub(crate) fn toggle_reply_mention(&mut self) {
        if let Some(reply) = self.reply.as_mut() {
            reply.mention = !reply.mention;
        }
    }

    pub(crate) fn cancel_edit(&mut self) {
        self.editing.set(None);
    }

    pub(crate) fn close_offers(&mut self) {
        self.offers.set(None);
    }

    pub(crate) fn step_offers(&mut self, down: bool) {
        if let Some(offers) = self.offers.as_mut() {
            offers.step(down);
        }
    }

    pub(crate) fn dismiss_refusal(&mut self) {
        self.refused.set(None);
    }

    pub(crate) fn remove_upload(&mut self, id: u64) {
        self.uploads.retain(|pending| pending.id != id);
        self.descriptions.remove(&id);
    }

    pub(crate) fn toggle_spoiler(&mut self, id: u64) {
        if let Some(pending) = self.uploads.iter_mut().find(|pending| pending.id == id) {
            pending.upload.spoiler = !pending.upload.spoiler;
        }
    }

    pub(crate) fn pick(&mut self, display: String, picked: Picked) {
        remember(&mut self.picked, display, picked);
    }

    pub(crate) fn picked(&self) -> &[(String, Picked)] {
        &self.picked
    }

    pub(super) fn take_uploads(&mut self) -> Vec<discord_core::Upload> {
        let uploads = std::mem::take(&mut *self.uploads);
        uploads
            .into_iter()
            .map(|pending| {
                let mut upload = pending.upload;
                upload.description = self
                    .descriptions
                    .remove(&pending.id)
                    .map(|display| display.text().trim().to_owned())
                    .filter(|display| !display.is_empty());
                upload
            })
            .collect()
    }

    fn admit(&mut self, admitted: Vec<Pending>, refused: Vec<String>) {
        for pending in &admitted {
            self.descriptions.insert(pending.id, TextDocument::new(""));
        }
        if !admitted.is_empty() {
            self.uploads.extend(admitted);
        }
        self.refused
            .set((!refused.is_empty()).then(|| refused.join("\n")));
    }

    pub(super) fn cleared(&mut self) {
        self.draft.clear();
        self.picked.clear();
        self.reply.set(None);
        self.offers.set(None);
        self.refused.set(None);
        self.typed_at = None;
    }

    pub(crate) fn leave(&mut self, from: Option<&Seat>, to: Option<&Seat>) {
        if let Some(from) = from {
            let saved = SeatDraft {
                text: self.draft.text(),
                reply: self.reply.take(),
                uploads: std::mem::take(&mut *self.uploads),
                picked: std::mem::take(&mut self.picked),
            };
            match saved.text.trim().is_empty() && saved.uploads.is_empty() && saved.reply.is_none()
            {
                true => self.drafts.remove(from),
                false => self.drafts.insert(from.clone(), saved),
            };
        }
        let restored = to.and_then(|to| self.drafts.remove(to)).unwrap_or_default();
        self.draft.set_text(restored.text);
        self.reply.set(restored.reply);
        self.uploads.set(restored.uploads);
        self.picked = restored.picked;
        self.editing.set(None);
        self.offers.set(None);
        self.refused.set(None);
        self.typed_at = None;
        self.cooldown.set(0);
    }
}

impl std::fmt::Debug for Composer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Composer")
            .field("reply", &*self.reply)
            .field("editing", &*self.editing)
            .field("uploads", &self.uploads.len())
            .finish_non_exhaustive()
    }
}

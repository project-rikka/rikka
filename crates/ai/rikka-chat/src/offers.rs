use std::time::Duration;

use crate::argument::{Argument, argument_at};
use crate::command::command_at;
use crate::mention::mention_at;
use crate::offer::Offer;
use crate::offering::{Offering, complete_in, query_at};
use crate::prelude::*;

pub const OFFER_DWELL: Duration = Duration::from_millis(50);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DraftChange {
    Command(String),
    Mention(u64),
    Argument(u64),
    Cleared,
    Nothing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AcceptedOffer {
    pub runs: bool,
}

#[derive(Debug, Default, State)]
pub struct Offers {
    offering: Tracked<Option<Offering>>,
    #[untracked]
    request_id: u64,
}

impl Offers {
    #[must_use]
    pub fn offering(&self) -> Option<&Offering> {
        self.offering.as_ref()
    }

    pub fn typed(&mut self, draft: &TextDocument) -> DraftChange {
        let text = draft.text();
        let caret = draft.caret();
        self.request_id += 1;
        if let Some(typing) = command_at(&text, caret) {
            return DraftChange::Command(typing.query().to_owned());
        }
        if mention_at(&text, caret).is_some() {
            return DraftChange::Mention(self.request_id);
        }
        if argument_at(&text, caret).is_some() {
            return DraftChange::Argument(self.request_id);
        }
        match self.dismiss() {
            true => DraftChange::Cleared,
            false => DraftChange::Nothing,
        }
    }

    pub fn command(&mut self, query: String, offers: Vec<Offer>) {
        self.offering.set(Offering::new(query, offers, 0));
    }

    #[must_use]
    pub fn pending_query(&self, request_id: u64, draft: &TextDocument) -> Option<String> {
        if request_id != self.request_id {
            return None;
        }
        let typing = mention_at(&draft.text(), draft.caret())?;
        Some(typing.query().to_owned())
    }

    #[must_use]
    pub fn pending_argument(&self, request_id: u64, draft: &TextDocument) -> Option<Argument> {
        if request_id != self.request_id {
            return None;
        }
        let text = draft.text();
        let caret = draft.caret();
        if mention_at(&text, caret).is_some() {
            return None;
        }
        argument_at(&text, caret).map(|typing| typing.argument)
    }

    pub fn found(&mut self, request_id: u64, draft: &TextDocument, found: Vec<Offer>) -> bool {
        if request_id != self.request_id {
            return false;
        }
        let renewed = query_at(&draft.text(), draft.caret())
            .and_then(|query| Offering::renewed(self.offering.as_ref(), query, found));
        if renewed.as_ref() == self.offering.as_ref() {
            return false;
        }
        self.offering.set(renewed);
        true
    }

    pub fn step(&mut self, down: bool) -> bool {
        match self.offering.as_mut() {
            Some(offering) => {
                offering.step(down);
                true
            }
            None => false,
        }
    }

    pub fn point(&mut self, at: usize) -> bool {
        let moves = self
            .offering
            .as_ref()
            .is_some_and(|offering| offering.at(at).is_some() && offering.selected() != at);
        if moves && let Some(offering) = self.offering.as_mut() {
            offering.pick(at);
        }
        moves
    }

    pub fn dismiss(&mut self) -> bool {
        let is_open = self.offering.is_some();
        if is_open {
            self.offering.set(None);
        }
        is_open
    }

    pub fn take(&mut self, at: usize, draft: &TextDocument) -> Option<AcceptedOffer> {
        let offer = self.offering.as_ref()?.at(at)?;
        let accepted = AcceptedOffer { runs: offer.runs() };
        complete_in(draft, offer.insertion());
        Some(accepted)
    }

    pub fn agree(&mut self, draft: &TextDocument) -> bool {
        let Some(offering) = self.offering.as_ref() else {
            return false;
        };
        match offering.agreed() {
            Some(agreed) => complete_in(draft, &agreed),
            None => match offering.at(offering.selected()) {
                Some(offer) => complete_in(draft, offer.insertion()),
                None => return false,
            },
        }
        true
    }
}

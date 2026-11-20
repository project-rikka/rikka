use std::sync::Arc;

use rikka_ui::TextDocument;

use crate::argument::argument_at;
use crate::command::command_at;
use crate::mention::mention_at;
use crate::offer::Offer;

#[derive(Clone, Debug, PartialEq)]
pub struct Offering {
    query: String,
    offers: Arc<[Offer]>,
    picked: usize,
}

impl Offering {
    #[must_use]
    pub(crate) fn new(query: String, offers: Vec<Offer>, selected: usize) -> Option<Offering> {
        (!offers.is_empty()).then(|| Offering {
            picked: selected.min(offers.len() - 1),
            query,
            offers: offers.into(),
        })
    }

    #[must_use]
    pub(crate) fn renewed(
        selected: Option<&Offering>,
        query: String,
        offers: Vec<Offer>,
    ) -> Option<Offering> {
        let selected_index = selected
            .and_then(|selected| selected.at(selected.selected()))
            .and_then(|current| offers.iter().position(|one| one == current))
            .unwrap_or(0);
        Offering::new(query, offers, selected_index)
    }

    pub fn offers(&self) -> &Arc<[Offer]> {
        &self.offers
    }

    pub fn selected(&self) -> usize {
        self.picked
    }

    pub fn at(&self, at: usize) -> Option<&Offer> {
        self.offers.get(at)
    }

    pub(crate) fn pick(&mut self, at: usize) -> bool {
        if at >= self.offers.len() || at == self.picked {
            return false;
        }
        self.picked = at;
        true
    }

    pub(crate) fn step(&mut self, down: bool) {
        let last = self.offers.len() - 1;
        self.picked = match (down, self.picked) {
            (true, at) if at == last => 0,
            (true, at) => at + 1,
            (false, 0) => last,
            (false, at) => at - 1,
        };
    }

    pub(crate) fn agreed(&self) -> Option<String> {
        let (first, rest) = self.offers.split_first()?;
        if rest.is_empty() {
            return None;
        }
        let mut agreed = first.spelt().to_owned();
        for other in rest {
            agreed.truncate(shared(&agreed, other.spelt()));
            if agreed.is_empty() {
                return None;
            }
        }
        (agreed.chars().count() > self.query.chars().count()).then(|| first.narrowed(&agreed))
    }
}

pub(crate) fn complete_in(draft: &TextDocument, completion: &str) {
    let text = draft.text();
    let caret = draft.caret();
    let Some((at, end)) = region(&text, caret) else {
        return;
    };
    draft.select(at, end);
    draft.insert(completion);
}

pub(crate) fn query_at(text: &str, caret: usize) -> Option<String> {
    if command_at(text, caret).is_some() {
        return None;
    }
    match mention_at(text, caret) {
        Some(typing) => Some(typing.query().to_owned()),
        None => argument_at(text, caret).map(|typing| typing.argument.typed().to_owned()),
    }
}

fn region(text: &str, caret: usize) -> Option<(usize, usize)> {
    if let Some(typing) = command_at(text, caret) {
        return Some((0, typing.end()));
    }
    if let Some(typing) = mention_at(text, caret) {
        return Some((typing.at(), typing.end()));
    }
    argument_at(text, caret).map(|typing| (typing.at, typing.end))
}

fn shared(one: &str, other: &str) -> usize {
    one.char_indices()
        .zip(other.chars())
        .take_while(|((_, letter), same)| letter == same)
        .map(|((at, letter), _)| at + letter.len_utf8())
        .last()
        .unwrap_or(0)
}

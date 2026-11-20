mod about;
mod elapsed;
mod full;
mod header;
mod identity;
mod look;
mod popout;

use discord_core::{SpaceId, UserId};
use rikka_app::prelude::*;

pub(crate) use full::{full, scrim};
pub(crate) use popout::{panel, popout};
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ProfileTarget {
    pub(crate) key: String,
    pub(crate) user: UserId,
    pub(crate) space: Option<SpaceId>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum Tab {
    #[default]
    About,
    Servers,
    Friends,
}

impl Tab {
    pub(crate) const ALL: [Tab; 3] = [Tab::About, Tab::Servers, Tab::Friends];
}

#[derive(State)]
pub(crate) struct Profiles {
    popout: Tracked<Option<(ProfileTarget, Anchor)>>,
    full: Tracked<Option<(ProfileTarget, Tab)>>,
    #[untracked]
    message: TextDocument,
    #[untracked]
    note: TextDocument,
    #[untracked]
    saved_note: Option<String>,
    note_loaded: Tracked<bool>,
}

impl Profiles {
    pub(crate) fn new() -> Profiles {
        Profiles {
            popout: Tracked::new(None),
            full: Tracked::new(None),
            message: TextDocument::new(""),
            note: TextDocument::new(""),
            saved_note: None,
            note_loaded: Tracked::new(false),
        }
    }

    pub(crate) fn popout(&self) -> Option<&(ProfileTarget, Anchor)> {
        self.popout.as_ref()
    }

    pub(crate) fn full(&self) -> Option<&(ProfileTarget, Tab)> {
        self.full.as_ref()
    }

    pub(crate) fn message(&self) -> &TextDocument {
        &self.message
    }

    pub(crate) fn note(&self) -> &TextDocument {
        &self.note
    }

    pub(crate) fn note_loaded(&self) -> bool {
        *self.note_loaded
    }

    pub(crate) fn show_popout(&mut self, target: ProfileTarget, anchor: Anchor) {
        if self.popout().is_none_or(|(shown, _)| *shown != target) {
            self.message.clear();
        }
        self.popout.set(Some((target, anchor)));
    }

    pub(crate) fn close_popout(&mut self) {
        self.popout.set(None);
    }

    pub(crate) fn show_full(&mut self, target: ProfileTarget, tab: Tab) {
        self.popout.set(None);
        self.note.clear();
        self.saved_note = None;
        self.note_loaded.set(false);
        self.full.set(Some((target, tab)));
    }

    pub(crate) fn show_tab(&mut self, tab: Tab) {
        let Some((target, shown)) = self.full().cloned() else {
            return;
        };
        if shown != tab {
            self.full.set(Some((target, tab)));
        }
    }

    pub(crate) fn close_full(&mut self) {
        self.full.set(None);
    }

    pub(crate) fn take_message(&mut self) -> String {
        let caption = self.message.text().trim().to_owned();
        self.message.clear();
        caption
    }

    pub(crate) fn on_note_loaded(&mut self, target: &ProfileTarget, note: String) {
        if self.full().is_none_or(|(shown, _)| shown != target) {
            return;
        }
        self.note.set_text(&note);
        self.saved_note = Some(note);
        self.note_loaded.set(true);
    }

    pub(crate) fn changed_note(&mut self) -> Option<String> {
        let saved = self.saved_note.as_mut()?;
        let current = self.note.text().trim().to_owned();
        if current == *saved {
            return None;
        }
        saved.clone_from(&current);
        Some(current)
    }
}

impl std::fmt::Debug for Profiles {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Profiles")
            .field("popout", &*self.popout)
            .field("full", &*self.full)
            .finish_non_exhaustive()
    }
}

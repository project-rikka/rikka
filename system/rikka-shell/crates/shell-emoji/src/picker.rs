use std::rc::Rc;
use std::sync::Arc;

use rikka_emoji::{Catalog, Emoji, Group, Tone};
use rikka_seam::{Request, SeamClient};
use rikka_ui::{State, TextDocument};
use shell_kit::{AimTarget, CLOSE, Recents};

use crate::CAPABILITY;
use crate::service::INSERT;
use crate::skin_tone::SkinTone;
use crate::tab::Tab;

#[derive(Debug, State)]
pub(crate) struct Picker {
    seam: SeamClient,
    catalogue: Arc<Catalog>,
    recents: Recents<String>,
    skin_tone: SkinTone,
    tab: Tab,
    query: TextDocument,
    shown: Rc<Vec<usize>>,
    toneable: bool,
    listing: u64,
    selected: usize,
    aim: Option<AimTarget>,
}

impl Picker {
    #[must_use]
    pub(crate) fn new(
        seam: SeamClient,
        recents: Recents<String>,
        skin_tone: SkinTone,
        aim: Option<AimTarget>,
    ) -> Picker {
        let catalogue = Catalog::current();
        Picker {
            seam,
            catalogue,
            recents,
            skin_tone,
            tab: Tab::Group(Group::Smileys),
            query: TextDocument::default(),
            shown: Rc::new(Vec::new()),
            toneable: false,
            listing: 0,
            selected: 0,
            aim,
        }
    }

    pub(crate) fn field(&self) -> &TextDocument {
        &self.query
    }

    pub(crate) fn tab(&self) -> Tab {
        self.tab
    }

    pub(crate) fn tone(&self) -> Tone {
        self.skin_tone.tone()
    }

    pub(crate) fn selected(&self) -> usize {
        self.selected
    }

    pub(crate) fn aim(&self) -> Option<AimTarget> {
        self.aim
    }

    pub(super) fn is_searching(&self) -> bool {
        !self.query.text().trim().is_empty()
    }

    pub(super) fn has_recents(&self) -> bool {
        !self.recents.is_empty()
    }

    pub(crate) fn count(&self) -> usize {
        self.shown.len()
    }

    pub(super) fn takes_tone(&self) -> bool {
        self.toneable
    }

    pub(crate) fn cells(&self) -> Cells {
        Cells {
            catalogue: Arc::clone(&self.catalogue),
            shown: Rc::clone(&self.shown),
            tone: self.tone(),
        }
    }

    pub(crate) fn showing(&self) -> Option<&Emoji> {
        self.row(self.selected)
    }

    pub(crate) fn swatch(&self, tone: Tone) -> &'static str {
        self.catalogue.sample(tone)
    }

    pub(crate) fn listing(&self) -> u64 {
        self.listing
    }

    pub(crate) fn start(&mut self) {
        if self.has_recents() {
            self.tab = Tab::Recent;
        }
        self.narrow();
    }

    fn row(&self, index: usize) -> Option<&Emoji> {
        self.catalogue.get(*self.shown.get(index)?)
    }

    pub(crate) fn narrow(&mut self) {
        let shown = match self.is_searching() {
            true => self.catalogue.search(&self.query.text()),
            false => self.listed(),
        };
        self.toneable = shown
            .iter()
            .filter_map(|index| self.catalogue.get(*index))
            .any(Emoji::supports_tone);
        self.shown = Rc::new(shown);
        self.listing = self.listing.wrapping_add(1);
        self.selected = 0;
    }

    fn listed(&self) -> Vec<usize> {
        match self.tab {
            Tab::Recent => self
                .recents
                .items()
                .iter()
                .filter_map(|text| self.catalogue.position(text))
                .collect(),
            Tab::Group(group) => self.catalogue.group_indices(group),
        }
    }

    pub(crate) fn walk(&mut self, delta: i32) {
        let Some(last) = self.count().checked_sub(1) else {
            return;
        };
        self.selected = self
            .selected
            .saturating_add_signed(delta as isize)
            .min(last);
    }

    pub(crate) fn show(&mut self, tab: Tab) {
        self.tab = tab;
        self.query = TextDocument::default();
        self.narrow();
    }

    pub(crate) fn activate(&self, index: usize) {
        let Some(emoji) = self.row(index) else {
            return;
        };
        self.recents.record(emoji.text.to_owned());
        let typed = emoji.with_tone(self.tone());
        self.seam
            .notify(Request::new(CAPABILITY, INSERT).encode(&typed));
    }

    pub(super) fn set_tone(&mut self, tone: Tone) {
        self.skin_tone.ask(&self.seam, tone);
    }

    pub(crate) fn forget(&mut self) {
        self.recents.clear();
        if self.tab == Tab::Recent {
            self.narrow();
        }
    }

    pub(crate) fn escape(&mut self) {
        if !self.is_searching() {
            return self.close();
        }
        self.query = TextDocument::default();
        self.narrow();
    }

    pub(crate) fn close(&self) {
        self.seam.notify(Request::new(CAPABILITY, CLOSE));
    }
}

#[derive(Debug)]
pub(crate) struct Cells {
    catalogue: Arc<Catalog>,
    shown: Rc<Vec<usize>>,
    tone: Tone,
}

impl Cells {
    pub(crate) fn len(&self) -> usize {
        self.shown.len()
    }

    pub(crate) fn get(&self, at: usize) -> &'static str {
        self.shown
            .get(at)
            .and_then(|index| self.catalogue.get(*index))
            .map_or("", |emoji| emoji.with_tone(self.tone))
    }
}

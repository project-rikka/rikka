use rikka_app::prelude::*;
use url::Url;

use crate::omnibox::picker::Picker;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Chip {
    pub(crate) engine: String,
    pub(crate) name: String,
    pub(crate) keyword: String,
}

#[derive(State)]
pub(crate) struct Omnibox {
    #[untracked]
    pub(crate) typed: String,
    #[untracked]
    pub(crate) original: String,
    #[untracked]
    pub(crate) url: Option<Url>,
    #[untracked]
    pub(crate) spaced: bool,
    chip: Tracked<Option<Chip>>,
    #[state]
    pub(crate) picker: Picker,
}

impl Omnibox {
    pub(crate) fn new() -> Omnibox {
        Omnibox {
            typed: String::new(),
            original: String::new(),
            url: None,
            spaced: false,
            chip: Tracked::new(None),
            picker: Picker::new(),
        }
    }

    pub(crate) fn begin(&mut self, original: String, url: Option<Url>) {
        self.typed = original.clone();
        self.original = original;
        self.url = url;
        self.set_chip(None);
        self.picker.reset();
    }

    pub(crate) fn end(&mut self) {
        self.set_chip(None);
        self.picker.reset();
    }

    pub(crate) fn chip(&self) -> Option<&Chip> {
        self.chip.as_ref()
    }

    pub(crate) fn set_chip(&mut self, chip: Option<Chip>) {
        self.chip.set(chip);
    }
}

impl std::fmt::Debug for Omnibox {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Omnibox")
            .field("typed", &self.typed)
            .finish_non_exhaustive()
    }
}

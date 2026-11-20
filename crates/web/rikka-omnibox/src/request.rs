use std::time::SystemTime;

use crate::classify::Classified;
use crate::engines::SearchEngines;
use crate::limits::Limits;
use crate::suggestion::SearchSuggestion;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Surface {
    #[default]
    AddressField,
    CommandBar,
}

#[derive(Debug, Clone)]
#[must_use]
pub struct Request<'a> {
    pub(crate) input: &'a Classified,
    pub(crate) engines: &'a SearchEngines,
    pub(crate) surface: Surface,
    pub(crate) just_deleted: bool,
    pub(crate) suggestions: &'a [SearchSuggestion],
    pub(crate) now: SystemTime,
    pub(crate) limits: Limits,
}

impl<'a> Request<'a> {
    pub fn new(input: &'a Classified, engines: &'a SearchEngines) -> Request<'a> {
        Request {
            input,
            engines,
            surface: Surface::AddressField,
            just_deleted: false,
            suggestions: &[],
            now: SystemTime::now(),
            limits: Limits::default(),
        }
    }

    pub fn surface(mut self, surface: Surface) -> Self {
        self.surface = surface;
        self
    }

    pub fn just_deleted(mut self, deleted: bool) -> Self {
        self.just_deleted = deleted;
        self
    }

    pub fn suggestions(mut self, suggestions: &'a [SearchSuggestion]) -> Self {
        self.suggestions = suggestions;
        self
    }

    pub fn now(mut self, now: SystemTime) -> Self {
        self.now = now;
        self
    }

    pub fn limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }
}

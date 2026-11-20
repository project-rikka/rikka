use std::ops::Range;

use crate::action::Action;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MatchKind {
    TypedUrl,
    TypedSearch,
    Suggestion,
    SuggestedUrl,
    OpenTab,
    Pinned,
    Favorite,
    History,
    Archived,
    Command,
}

impl MatchKind {
    pub(crate) const COUNT: usize = 10;

    pub(crate) fn index(self) -> usize {
        self as usize
    }

    pub(crate) fn rank(self) -> u8 {
        match self {
            MatchKind::TypedUrl | MatchKind::TypedSearch => 0,
            MatchKind::OpenTab => 1,
            MatchKind::Pinned => 2,
            MatchKind::Favorite => 3,
            MatchKind::History => 4,
            MatchKind::Archived => 5,
            MatchKind::SuggestedUrl => 6,
            MatchKind::Command => 7,
            MatchKind::Suggestion => 8,
        }
    }

    pub(crate) fn is_tab(self) -> bool {
        matches!(
            self,
            MatchKind::OpenTab | MatchKind::Pinned | MatchKind::Favorite
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match<K> {
    pub kind: MatchKind,
    pub title: String,
    pub url: String,
    pub action: Action<K>,
    pub title_highlights: Vec<Range<usize>>,
    pub url_highlights: Vec<Range<usize>>,
    pub relevance: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ranked<K> {
    pub matches: Vec<Match<K>>,
    pub inline: Option<String>,
}

impl<K> Ranked<K> {
    #[must_use]
    pub fn default_match(&self) -> Option<&Match<K>> {
        self.matches.first()
    }
}

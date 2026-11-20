use crate::ranked::MatchKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub struct Limits {
    total: usize,
    caps: [usize; MatchKind::COUNT],
}

impl Limits {
    pub fn total(mut self, total: usize) -> Self {
        self.total = total;
        self
    }

    pub fn cap(mut self, kind: MatchKind, cap: usize) -> Self {
        self.caps[kind.index()] = cap;
        self
    }

    #[must_use]
    pub fn total_limit(&self) -> usize {
        self.total
    }

    #[must_use]
    pub fn cap_of(&self, kind: MatchKind) -> usize {
        self.caps[kind.index()]
    }
}

impl Default for Limits {
    fn default() -> Limits {
        let mut limits = Limits {
            total: 10,
            caps: [0; MatchKind::COUNT],
        };
        for (kind, cap) in [
            (MatchKind::TypedUrl, 1),
            (MatchKind::TypedSearch, 1),
            (MatchKind::Suggestion, 5),
            (MatchKind::SuggestedUrl, 1),
            (MatchKind::OpenTab, 3),
            (MatchKind::Pinned, 3),
            (MatchKind::Favorite, 3),
            (MatchKind::History, 5),
            (MatchKind::Archived, 2),
            (MatchKind::Command, 3),
        ] {
            limits.caps[kind.index()] = cap;
        }
        limits
    }
}

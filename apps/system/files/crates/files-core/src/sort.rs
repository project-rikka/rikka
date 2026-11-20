use std::cmp::Ordering;

use rikka_format::natural_cmp;

use crate::entry::Entry;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SortKey {
    #[default]
    Name,
    Modified,
    Kind,
    Size,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SortOrder {
    #[default]
    Ascending,
    Descending,
}

impl SortOrder {
    #[must_use]
    pub fn reversed(self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }

    fn apply(self, ordering: Ordering) -> Ordering {
        match self {
            Self::Ascending => ordering,
            Self::Descending => ordering.reverse(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Sorting {
    pub key: SortKey,
    pub order: SortOrder,
    pub folders_first: bool,
}

impl Sorting {
    #[must_use]
    pub fn new() -> Self {
        Self {
            key: SortKey::Name,
            order: SortOrder::Ascending,
            folders_first: true,
        }
    }

    #[must_use]
    pub fn toggled(self, key: SortKey) -> Self {
        if self.key == key {
            return Self {
                order: self.order.reversed(),
                ..self
            };
        }
        let order = match key {
            SortKey::Name | SortKey::Kind => SortOrder::Ascending,
            SortKey::Modified | SortKey::Size => SortOrder::Descending,
        };
        Self { key, order, ..self }
    }

    pub fn apply(self, entries: &mut [Entry]) {
        entries.sort_by(|one, other| self.cmp(one, other));
    }

    #[must_use]
    pub fn cmp(self, one: &Entry, other: &Entry) -> Ordering {
        if self.folders_first {
            let grouped = other.is_directory().cmp(&one.is_directory());
            if grouped != Ordering::Equal {
                return grouped;
            }
        }
        let by_key = match self.key {
            SortKey::Name => Ordering::Equal,
            SortKey::Modified => one.modified.cmp(&other.modified),
            SortKey::Size => one.size.cmp(&other.size),
            SortKey::Kind => (one.family() as u8)
                .cmp(&(other.family() as u8))
                .then_with(|| one.mime.cmp(&other.mime)),
        };
        self.order
            .apply(by_key.then_with(|| natural_cmp(&one.name, &other.name)))
    }
}

impl Default for Sorting {
    fn default() -> Self {
        Self::new()
    }
}

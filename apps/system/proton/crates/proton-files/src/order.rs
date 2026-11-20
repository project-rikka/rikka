use std::cmp::Ordering;

use rikka_format::natural_cmp;

use crate::entry::Entry;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortKey {
    #[default]
    Name,
    Modified,
    Kind,
    Size,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Order {
    sorted: SortKey,
    descending: bool,
}

impl Order {
    #[must_use]
    pub fn sorted(self) -> SortKey {
        self.sorted
    }

    #[must_use]
    pub fn is_descending(self) -> bool {
        self.descending
    }

    #[must_use]
    pub fn toggled(self, sorted: SortKey) -> Order {
        match self.sorted == sorted {
            true => Order {
                sorted,
                descending: !self.descending,
            },
            false => Order {
                sorted,
                descending: sorted != SortKey::Name,
            },
        }
    }

    pub fn arrange(self, entries: &mut [Entry]) {
        entries.sort_by(|one, other| self.compare(one, other));
    }

    #[must_use]
    pub fn is_sorted(self, entries: &[Entry]) -> bool {
        entries.is_sorted_by(|one, other| self.compare(one, other) != Ordering::Greater)
    }

    fn compare(self, one: &Entry, other: &Entry) -> Ordering {
        let first = other.folder.cmp(&one.folder);
        if first != Ordering::Equal {
            return first;
        }
        let by = match self.sorted {
            SortKey::Name => natural_cmp(&one.name, &other.name),
            SortKey::Modified => one.modified.cmp(&other.modified),
            SortKey::Kind => natural_cmp(one.kind_key(), other.kind_key())
                .then_with(|| natural_cmp(&one.name, &other.name)),
            SortKey::Size => one.size.cmp(&other.size),
        };
        match self.descending {
            true => by.reverse(),
            false => by,
        }
    }
}

use serde::{Deserialize, Serialize};

use crate::entry::Entry;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SortKey {
    #[default]
    Recent,
    TitleAscending,
    TitleDescending,
    Oldest,
    Newest,
}

impl SortKey {
    pub const ALL: [SortKey; 5] = [
        SortKey::Recent,
        SortKey::TitleAscending,
        SortKey::TitleDescending,
        SortKey::Oldest,
        SortKey::Newest,
    ];

    #[must_use]
    pub fn compare(self, one: &Entry, other: &Entry) -> std::cmp::Ordering {
        match self {
            SortKey::Recent => touched(other).cmp(&touched(one)),
            SortKey::TitleAscending => rikka_format::natural_cmp(&one.title, &other.title),
            SortKey::TitleDescending => rikka_format::natural_cmp(&other.title, &one.title),
            SortKey::Oldest => one.created.cmp(&other.created),
            SortKey::Newest => other.created.cmp(&one.created),
        }
    }
}

fn touched(entry: &Entry) -> jiff::Timestamp {
    entry.used.max(entry.modified).max(entry.created)
}

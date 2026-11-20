use crate::opened::OpenedItem;
use crate::values::Field;
use jiff::Timestamp;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Change {
    Created,
    Title,
    Identifier,
    Password,
    Code,
    Websites,
    Note,
    Fields,
    Card,
    Wifi,
    Ssh,
}

#[derive(Clone, Debug)]
pub struct HistoryEntry {
    pub revision: i64,
    pub at: Timestamp,
    pub changes: Vec<Change>,
    pub opened: OpenedItem,
}

impl HistoryEntry {
    #[must_use]
    pub fn is_first(&self) -> bool {
        self.changes.first() == Some(&Change::Created)
    }
}

#[must_use]
pub(crate) fn history_entries(revisions: Vec<OpenedItem>) -> Vec<HistoryEntry> {
    let mut sorted: Vec<OpenedItem> = revisions;
    sorted.sort_by_key(|opened| opened.revision);

    let mut history_entries: Vec<HistoryEntry> = Vec::with_capacity(sorted.len());
    for (at, opened) in sorted.iter().enumerate() {
        let changes = match at.checked_sub(1).and_then(|before| sorted.get(before)) {
            Some(before) => changes_between(before, opened),
            None => vec![Change::Created],
        };
        history_entries.push(HistoryEntry {
            revision: opened.revision,
            at: opened.modified,
            changes,
            opened: opened.clone(),
        });
    }

    history_entries.reverse();
    history_entries
}

fn changes_between(before: &OpenedItem, after: &OpenedItem) -> Vec<Change> {
    let mut changes = Vec::new();
    let mut record = |changed: bool, change: Change| {
        if changed {
            changes.push(change);
        }
    };

    record(before.title != after.title, Change::Title);
    record(
        before.identifier() != after.identifier(),
        Change::Identifier,
    );
    record(before.password != after.password, Change::Password);
    record(before.totp != after.totp, Change::Code);
    record(before.urls != after.urls, Change::Websites);
    record(before.note != after.note, Change::Note);
    record(
        field_pairs(&before.fields) != field_pairs(&after.fields),
        Change::Fields,
    );
    record(before.card != after.card, Change::Card);
    record(before.wifi != after.wifi, Change::Wifi);
    record(before.ssh != after.ssh, Change::Ssh);

    changes
}

fn field_pairs(fields: &[Field]) -> Vec<(&str, String)> {
    fields
        .iter()
        .map(|field| (field.name.as_str(), field.value.text()))
        .collect()
}

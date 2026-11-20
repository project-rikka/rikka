use jiff::Timestamp;
use std::collections::HashMap;

use rikka_file_format::{BinaryFormat, Reader};
use rikka_proton_api::EventAction;
use rikka_proton_pgp::SymmetricKey;
use serde::{Deserialize, Serialize};

use crate::decrypted::DecryptedMessage;
use crate::error::{Error, Result};
use crate::event::Event;
use crate::identifier::MessageId;
use crate::message::{Message, MessageMetadata};
use crate::search::entry::{Entry, StoredEntry};
use crate::search::normalize::normalize;
use crate::search::query::SearchQuery;

const FORMAT: BinaryFormat = BinaryFormat::new("mail search index", *b"RKMAILIX");

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Cursor {
    #[serde(with = "rikka_proton_api::unix_seconds")]
    pub(crate) time: Timestamp,
    pub(crate) id: MessageId,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Progress {
    complete: bool,
    cursor: Option<Cursor>,
}

#[derive(Serialize, Deserialize)]
struct SerializedIndex {
    progress: Progress,
    entries: Vec<StoredEntry>,
}

#[derive(Debug, Clone, Default)]
pub struct SearchIndex {
    entries: HashMap<MessageId, Entry>,
    progress: Progress,
}

impl SearchIndex {
    #[must_use]
    pub fn new() -> SearchIndex {
        SearchIndex::default()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[must_use]
    pub fn contains(&self, id: &MessageId) -> bool {
        self.entries.contains_key(id)
    }

    #[must_use]
    pub fn has_content(&self, id: &MessageId) -> bool {
        self.entries
            .get(id)
            .is_some_and(|entry| entry.content().is_some())
    }

    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.progress.complete
    }

    #[must_use]
    pub fn metadata(&self, id: &MessageId) -> Option<&MessageMetadata> {
        self.entries.get(id).map(Entry::metadata)
    }

    pub fn insert(&mut self, metadata: MessageMetadata, text: Option<&str>) {
        let content = text.map(normalize);
        self.entries
            .insert(metadata.id().clone(), Entry::new(metadata, content));
    }

    pub fn index(&mut self, message: &Message, decrypted: &DecryptedMessage) {
        self.insert(message.metadata().clone(), Some(decrypted.content().text()));
    }

    pub fn remove(&mut self, id: &MessageId) -> bool {
        self.entries.remove(id).is_some()
    }

    pub fn apply(&mut self, event: &Event) -> Vec<MessageId> {
        let mut needs_content = Vec::new();

        for change in event.messages() {
            let id = change.id();
            match change.action() {
                EventAction::Delete => {
                    self.entries.remove(id);
                }
                action => {
                    let Some(metadata) = change.message() else {
                        continue;
                    };
                    let draft_updated = action == EventAction::Update && metadata.is_draft();

                    match self.entries.get_mut(id) {
                        Some(entry) => {
                            entry.refresh(metadata.clone());
                            if draft_updated {
                                entry.clear_content();
                                needs_content.push(id.clone());
                            }
                        }
                        None => {
                            self.entries
                                .insert(id.clone(), Entry::new(metadata.clone(), None));
                            needs_content.push(id.clone());
                        }
                    }
                }
            }
        }

        needs_content
    }

    #[must_use]
    pub fn missing_content(&self) -> Vec<MessageId> {
        self.entries
            .iter()
            .filter(|(_, entry)| entry.content().is_none())
            .map(|(id, _)| id.clone())
            .collect()
    }

    #[must_use]
    pub fn search(&self, query: &SearchQuery) -> Vec<&MessageMetadata> {
        let now = Timestamp::now();

        let mut results: Vec<&MessageMetadata> = self
            .entries
            .values()
            .filter(|entry| query.matches(entry, now))
            .map(Entry::metadata)
            .collect();

        results.sort_by(|left, right| {
            let ordering = (left.time(), left.order()).cmp(&(right.time(), right.order()));
            match query.is_oldest_first() {
                true => ordering,
                false => ordering.reverse(),
            }
        });

        if let Some(max_results) = query.max_results() {
            results.truncate(max_results);
        }
        results
    }

    pub fn seal(&self, key: &SymmetricKey) -> Result<Vec<u8>> {
        let serialized = SerializedIndex {
            progress: self.progress.clone(),
            entries: self.entries.values().map(Entry::to_stored).collect(),
        };
        let plain = serde_json::to_vec(&serialized).map_err(|source| Error::Encoding {
            what: "search index",
            source,
        })?;
        let header = FORMAT.header();
        let sealed = key.seal(&plain, header)?;

        let mut blob = Vec::with_capacity(header.len() + sealed.len());
        blob.extend_from_slice(&header);
        blob.extend_from_slice(&sealed);
        Ok(blob)
    }

    pub fn open(blob: &[u8], key: &SymmetricKey) -> Result<SearchIndex> {
        let mut input = Reader::new(blob);
        FORMAT
            .read_header(&mut input)
            .map_err(|_| Error::IndexDecryption)?;
        let sealed = input.rest();
        let header = FORMAT.header();
        let plain = key
            .open(sealed, header)
            .map_err(|_| Error::IndexDecryption)?;
        let serialized: SerializedIndex =
            serde_json::from_slice(&plain).map_err(|_| Error::IndexDecryption)?;

        Ok(SearchIndex {
            entries: serialized
                .entries
                .into_iter()
                .map(Entry::from_stored)
                .map(|entry| (entry.metadata().id().clone(), entry))
                .collect(),
            progress: serialized.progress,
        })
    }

    pub(crate) fn cursor(&self) -> Option<&Cursor> {
        self.progress.cursor.as_ref()
    }

    pub(crate) fn advance(&mut self, cursor: Cursor) {
        self.progress.cursor = Some(cursor);
    }

    pub(crate) fn finish(&mut self) {
        self.progress.complete = true;
    }
}

use jiff::Timestamp;
use rikka_proton_authenticator::{Entry, EntryState, LocalEntry, Otp, RemoteEntry};
use serde::{Deserialize, Serialize};

use crate::code::Code;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum State {
    #[default]
    Synced,
    Pending,
    PendingDelete,
}

impl State {
    #[must_use]
    pub fn to_entry_state(self) -> EntryState {
        match self {
            State::Synced => EntryState::Synced,
            State::Pending => EntryState::PendingSync,
            State::PendingDelete => EntryState::PendingDelete,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct StoredCode {
    pub(crate) id: String,
    #[serde(rename = "uri", with = "otp_uri")]
    pub(crate) otp: Otp,
    pub(crate) note: Option<String>,
    pub(crate) remote: Option<String>,
    pub(crate) revision: u32,
    pub(crate) state: State,
    pub(crate) position: u32,
    #[serde(with = "rikka_proton_api::unix_seconds")]
    pub(crate) modified: Timestamp,
    #[serde(with = "rikka_proton_api::unix_seconds::optional")]
    pub(crate) edited: Option<Timestamp>,
}

impl StoredCode {
    #[must_use]
    pub fn new(
        id: String,
        otp: &Otp,
        note: Option<String>,
        at: Timestamp,
        position: u32,
    ) -> StoredCode {
        StoredCode {
            id,
            otp: otp.clone(),
            note,
            remote: None,
            revision: 0,
            state: State::Pending,
            position,
            modified: at,
            edited: Some(at),
        }
    }

    #[must_use]
    pub fn from_remote(remote: &RemoteEntry) -> StoredCode {
        StoredCode {
            id: remote.entry.id.clone(),
            otp: remote.entry.content.clone(),
            note: remote.entry.note.clone(),
            remote: Some(remote.remote_id.clone()),
            revision: remote.revision,
            state: State::Synced,
            position: remote.position.unwrap_or_default(),
            modified: remote.modified_at,
            edited: None,
        }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn otp(&self) -> &Otp {
        &self.otp
    }

    #[must_use]
    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }

    #[must_use]
    pub fn state(&self) -> State {
        self.state
    }

    #[must_use]
    pub fn position(&self) -> u32 {
        self.position
    }

    #[must_use]
    pub fn issuer(&self) -> &str {
        self.otp.issuer().unwrap_or_default()
    }

    #[must_use]
    pub fn label(&self) -> &str {
        self.otp.label().unwrap_or_default()
    }

    #[must_use]
    pub fn title(&self) -> &str {
        match self.issuer() {
            "" => self.label(),
            issuer => issuer,
        }
    }

    #[must_use]
    pub fn subtitle(&self) -> &str {
        match self.issuer() {
            "" => "",
            _ => self.label(),
        }
    }

    #[must_use]
    pub fn code_at(&self, at: Timestamp) -> Option<Code> {
        Code::at(&self.otp, at)
    }

    #[must_use]
    pub fn matches(&self, keyword: &str) -> bool {
        let keyword = keyword.trim().to_lowercase();
        if keyword.is_empty() {
            return true;
        }
        self.issuer().to_lowercase().contains(&keyword)
            || self.label().to_lowercase().contains(&keyword)
            || self
                .note
                .as_deref()
                .is_some_and(|note| note.to_lowercase().contains(&keyword))
    }

    #[must_use]
    pub fn entry(&self) -> Entry {
        let entry = Entry::new(self.id.clone(), self.otp.clone());
        match &self.note {
            Some(note) => entry.with_note(note.clone()),
            None => entry,
        }
    }

    #[must_use]
    pub fn local(&self) -> LocalEntry {
        let local = LocalEntry::new(
            self.entry(),
            self.state.to_entry_state(),
            self.position,
            self.modified,
        );
        match self.edited {
            Some(edited) => local.with_edited_at(edited),
            None => local,
        }
    }
}

impl std::fmt::Debug for StoredCode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StoredCode")
            .field("id", &self.id)
            .field("issuer", &self.issuer())
            .field("label", &self.label())
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

mod otp_uri {
    use rikka_proton_authenticator::Otp;
    use serde::de::Error as _;
    use serde::{Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S: Serializer>(otp: &Otp, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&otp.to_uri())
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Otp, D::Error> {
        let uri = String::deserialize(deserializer)?;
        Otp::parse(&uri).map_err(D::Error::custom)
    }
}

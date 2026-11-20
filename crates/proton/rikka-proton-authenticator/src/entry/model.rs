use crate::code::Codes;
use crate::entry::non_empty;
use crate::entry::otp::Otp;
use crate::entry::proto::{
    AuthenticatorEntry, AuthenticatorEntryContent, AuthenticatorEntryContentSteam,
    AuthenticatorEntryContentTotp, AuthenticatorEntryMetadata, EntryContent,
};
use crate::entry::steam::Steam;
use crate::error::{Error, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: String,
    pub note: Option<String>,
    pub content: Otp,
}

impl Entry {
    pub fn new(id: impl Into<String>, content: Otp) -> Entry {
        Entry {
            id: id.into(),
            note: None,
            content,
        }
    }

    pub fn from_uri(id: impl Into<String>, uri: &str) -> Result<Entry> {
        Ok(Entry::new(id, Otp::parse(uri)?))
    }

    #[must_use]
    pub fn with_note(self, note: impl Into<String>) -> Entry {
        Entry {
            note: Some(note.into()),
            ..self
        }
    }

    pub fn decode(bytes: &[u8]) -> Result<Entry> {
        let entry = <AuthenticatorEntry as prost::Message>::decode(bytes)
            .map_err(|error| Error::malformed("entry", error))?;
        Entry::try_from(entry)
    }

    pub fn encode(&self) -> Vec<u8> {
        prost::Message::encode_to_vec(&AuthenticatorEntry::from(self))
    }

    pub fn to_uri(&self) -> String {
        self.content.to_uri()
    }

    pub fn name(&self) -> Option<&str> {
        self.content.label()
    }

    pub fn issuer(&self) -> Option<&str> {
        self.content.issuer()
    }

    pub fn secret(&self) -> &str {
        self.content.secret()
    }

    pub fn period(&self) -> Option<u16> {
        self.content.period()
    }

    pub fn remaining_at(&self, at: jiff::Timestamp) -> Option<u16> {
        self.content.remaining_at(at)
    }

    pub fn code_at(&self, at: jiff::Timestamp) -> Result<String> {
        self.content.code_at(at)
    }

    pub fn codes_at(&self, at: jiff::Timestamp) -> Result<Codes> {
        self.content.codes_at(at)
    }
}

impl From<&Entry> for AuthenticatorEntry {
    fn from(entry: &Entry) -> AuthenticatorEntry {
        let content = match &entry.content {
            Otp::Steam(steam) => EntryContent::Steam(AuthenticatorEntryContentSteam {
                secret: steam.secret.clone(),
            }),
            content => EntryContent::Totp(AuthenticatorEntryContentTotp {
                uri: content.to_uri(),
            }),
        };
        AuthenticatorEntry {
            metadata: Some(AuthenticatorEntryMetadata {
                name: entry.name().unwrap_or_default().to_owned(),
                note: entry.note.clone().unwrap_or_default(),
                id: entry.id.clone(),
            }),
            content: Some(AuthenticatorEntryContent {
                content: Some(content),
            }),
        }
    }
}

impl TryFrom<AuthenticatorEntry> for Entry {
    type Error = Error;

    fn try_from(entry: AuthenticatorEntry) -> Result<Entry> {
        let metadata = entry.metadata.unwrap_or_default();
        let content = entry.content.and_then(|content| content.content);
        let content = match content.ok_or(Error::EmptyEntry)? {
            EntryContent::Totp(totp) => Otp::parse(&totp.uri)?,
            EntryContent::Steam(steam) => Otp::Steam(Steam {
                secret: steam.secret,
                name: non_empty(&metadata.name),
            }),
        };
        Ok(Entry {
            id: metadata.id,
            note: non_empty(&metadata.note),
            content,
        })
    }
}

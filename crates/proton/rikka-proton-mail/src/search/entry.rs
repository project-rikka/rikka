use serde::{Deserialize, Serialize};

use crate::address::MailAddress;
use crate::message::MessageMetadata;
use crate::search::normalize::normalize;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StoredEntry {
    #[serde(rename = "m")]
    metadata: MessageMetadata,
    #[serde(rename = "c", default, skip_serializing_if = "Option::is_none")]
    content: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct Entry {
    metadata: MessageMetadata,
    content: Option<String>,
    subject: String,
    senders: Vec<String>,
    recipients: Vec<String>,
}

impl Entry {
    pub(crate) fn new(metadata: MessageMetadata, content: Option<String>) -> Entry {
        let subject = normalize(metadata.subject());
        let senders = normalized_addresses(std::slice::from_ref(metadata.sender()));
        let recipients = normalized_addresses(
            &metadata
                .to()
                .iter()
                .chain(metadata.cc())
                .chain(metadata.bcc())
                .cloned()
                .collect::<Vec<_>>(),
        );

        Entry {
            metadata,
            content,
            subject,
            senders,
            recipients,
        }
    }

    pub(crate) fn from_stored(stored: StoredEntry) -> Entry {
        Entry::new(stored.metadata, stored.content)
    }

    pub(crate) fn to_stored(&self) -> StoredEntry {
        StoredEntry {
            metadata: self.metadata.clone(),
            content: self.content.clone(),
        }
    }

    pub(crate) fn metadata(&self) -> &MessageMetadata {
        &self.metadata
    }

    pub(crate) fn content(&self) -> Option<&str> {
        self.content.as_deref()
    }

    pub(crate) fn subject(&self) -> &str {
        &self.subject
    }

    pub(crate) fn senders(&self) -> &[String] {
        &self.senders
    }

    pub(crate) fn recipients(&self) -> &[String] {
        &self.recipients
    }

    pub(crate) fn refresh(&mut self, metadata: MessageMetadata) {
        let content = self.content.take();
        *self = Entry::new(metadata, content);
    }

    pub(crate) fn clear_content(&mut self) {
        self.content = None;
    }
}

fn normalized_addresses(addresses: &[MailAddress]) -> Vec<String> {
    addresses
        .iter()
        .flat_map(|address| [address.address(), address.name()])
        .filter(|part| !part.is_empty())
        .map(normalize)
        .collect()
}

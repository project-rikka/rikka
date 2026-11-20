use jiff::Timestamp;
use rikka_proton_account::AddressId;

use crate::identifier::LabelId;
use crate::search::entry::Entry;
use crate::search::normalize::{keywords, normalize};

#[must_use]
#[derive(Debug, Clone, Default)]
pub struct SearchQuery {
    keywords: Vec<String>,
    from: Option<String>,
    to: Option<String>,
    subject: Option<String>,
    labels: Vec<LabelId>,
    address: Option<AddressId>,
    begin: Option<Timestamp>,
    end: Option<Timestamp>,
    attachments: Option<bool>,
    unread: Option<bool>,
    oldest_first: bool,
    limit: Option<usize>,
}

impl SearchQuery {
    pub fn new() -> SearchQuery {
        SearchQuery::default()
    }

    pub fn keywords(mut self, text: &str) -> Self {
        self.keywords = keywords(text);
        self
    }

    pub fn from(mut self, sender: &str) -> Self {
        self.from = normalized_filter(sender);
        self
    }

    pub fn to(mut self, recipient: &str) -> Self {
        self.to = normalized_filter(recipient);
        self
    }

    pub fn subject(mut self, subject: &str) -> Self {
        self.subject = normalized_filter(subject);
        self
    }

    pub fn label(mut self, label: &LabelId) -> Self {
        self.labels.push(label.clone());
        self
    }

    pub fn address(mut self, address: &AddressId) -> Self {
        self.address = Some(address.clone());
        self
    }

    pub fn begin(mut self, at: Timestamp) -> Self {
        self.begin = Some(at);
        self
    }

    pub fn end(mut self, at: Timestamp) -> Self {
        self.end = Some(at);
        self
    }

    pub fn with_attachments(mut self, attachments: bool) -> Self {
        self.attachments = Some(attachments);
        self
    }

    pub fn unread(mut self, unread: bool) -> Self {
        self.unread = Some(unread);
        self
    }

    pub fn oldest_first(mut self) -> Self {
        self.oldest_first = true;
        self
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    #[must_use]
    pub fn parsed_keywords(&self) -> &[String] {
        &self.keywords
    }

    pub(crate) fn is_oldest_first(&self) -> bool {
        self.oldest_first
    }

    pub(crate) fn max_results(&self) -> Option<usize> {
        self.limit
    }

    pub(crate) fn matches(&self, entry: &Entry, now: Timestamp) -> bool {
        self.metadata_matches(entry, now) && self.keywords_match(entry)
    }

    fn metadata_matches(&self, entry: &Entry, now: Timestamp) -> bool {
        let metadata = entry.metadata();

        if !self.labels.is_empty() && !self.labels.iter().any(|label| metadata.has_label(label)) {
            return false;
        }
        if self
            .address
            .as_ref()
            .is_some_and(|address| metadata.address_id() != address)
        {
            return false;
        }
        let expiry = metadata.expiration_time();
        if expiry > Timestamp::UNIX_EPOCH && expiry <= now {
            return false;
        }
        if self.begin.is_some_and(|begin| metadata.time() < begin)
            || self.end.is_some_and(|end| metadata.time() > end)
        {
            return false;
        }
        if self
            .from
            .as_deref()
            .is_some_and(|from| !entry.senders().iter().any(|address| address.contains(from)))
        {
            return false;
        }
        if self.to.as_deref().is_some_and(|to| {
            !entry
                .recipients()
                .iter()
                .any(|address| address.contains(to))
        }) {
            return false;
        }
        if self
            .subject
            .as_deref()
            .is_some_and(|subject| !entry.subject().contains(subject))
        {
            return false;
        }
        if self
            .attachments
            .is_some_and(|has_attachments| (metadata.attachment_count() > 0) != has_attachments)
        {
            return false;
        }
        if self
            .unread
            .is_some_and(|unread| metadata.is_unread() != unread)
        {
            return false;
        }
        true
    }

    fn keywords_match(&self, entry: &Entry) -> bool {
        self.keywords.iter().all(|keyword| {
            entry.subject().contains(keyword.as_str())
                || entry
                    .senders()
                    .iter()
                    .any(|address| address.contains(keyword.as_str()))
                || entry
                    .recipients()
                    .iter()
                    .any(|address| address.contains(keyword.as_str()))
                || entry
                    .content()
                    .is_some_and(|content| content.contains(keyword.as_str()))
        })
    }
}

fn normalized_filter(text: &str) -> Option<String> {
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| normalize(trimmed))
}

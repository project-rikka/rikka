use serde::Deserialize;

use crate::message::{DRAFT, IMPORTANT, INBOX, SENT, SPAM, STARRED, TRASH};

const SYSTEM: &str = "system";
const HIDDEN: &str = "labelHide";
const ALL_MAIL: &str = "";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mailbox {
    Inbox,
    Starred,
    Important,
    Sent,
    Drafts,
    AllMail,
    Spam,
    Trash,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    label_list_visibility: String,
    #[serde(default)]
    pub threads_unread: u32,
    #[serde(default)]
    pub threads_total: u32,
}

impl Mailbox {
    pub const ALL: [Mailbox; 8] = [
        Mailbox::Inbox,
        Mailbox::Starred,
        Mailbox::Important,
        Mailbox::Sent,
        Mailbox::Drafts,
        Mailbox::AllMail,
        Mailbox::Spam,
        Mailbox::Trash,
    ];

    #[must_use]
    pub const fn label(self) -> Option<&'static str> {
        match self {
            Mailbox::Inbox => Some(INBOX),
            Mailbox::Starred => Some(STARRED),
            Mailbox::Important => Some(IMPORTANT),
            Mailbox::Sent => Some(SENT),
            Mailbox::Drafts => Some(DRAFT),
            Mailbox::AllMail => None,
            Mailbox::Spam => Some(SPAM),
            Mailbox::Trash => Some(TRASH),
        }
    }

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self.label() {
            Some(label) => label,
            None => ALL_MAIL,
        }
    }

    #[must_use]
    pub const fn counts_unread(self) -> bool {
        matches!(self, Mailbox::Inbox | Mailbox::Spam)
    }
}

impl Label {
    #[must_use]
    pub fn is_user(&self) -> bool {
        self.kind != SYSTEM
    }

    #[must_use]
    pub fn is_listed(&self) -> bool {
        self.is_user() && self.label_list_visibility != HIDDEN
    }

    #[must_use]
    pub fn leaf(&self) -> &str {
        self.name.rsplit('/').next().unwrap_or(&self.name)
    }

    #[must_use]
    pub fn depth(&self) -> usize {
        self.name.matches('/').count()
    }
}

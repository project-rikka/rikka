use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use crate::address::MailAddress;
use crate::identifier::{ConversationId, LabelId};
use crate::message::MessageMetadata;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConversationLabel {
    #[serde(rename = "ID", default)]
    id: LabelId,
    #[serde(
        rename = "ContextTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    time: Timestamp,
    #[serde(rename = "ContextSize", default)]
    size: i64,
    #[serde(rename = "ContextNumMessages", default)]
    messages: i64,
    #[serde(rename = "ContextNumUnread", default)]
    unread: i64,
    #[serde(rename = "ContextNumAttachments", default)]
    attachments: i64,
    #[serde(
        rename = "ContextExpirationTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    expiration_time: Timestamp,
}

impl ConversationLabel {
    #[must_use]
    pub fn id(&self) -> &LabelId {
        &self.id
    }

    #[must_use]
    pub fn time(&self) -> Timestamp {
        self.time
    }

    #[must_use]
    pub fn size(&self) -> i64 {
        self.size
    }

    #[must_use]
    pub fn messages(&self) -> i64 {
        self.messages
    }

    #[must_use]
    pub fn unread(&self) -> i64 {
        self.unread
    }

    #[must_use]
    pub fn attachments(&self) -> i64 {
        self.attachments
    }

    #[must_use]
    pub fn expiration_time(&self) -> Timestamp {
        self.expiration_time
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Conversation {
    #[serde(rename = "ID", default)]
    id: ConversationId,
    #[serde(rename = "Subject", default)]
    subject: String,
    #[serde(rename = "Senders", default)]
    senders: Vec<MailAddress>,
    #[serde(rename = "Recipients", default)]
    recipients: Vec<MailAddress>,
    #[serde(rename = "Labels", default)]
    labels: Vec<ConversationLabel>,
    #[serde(rename = "NumMessages", default)]
    messages: i64,
    #[serde(rename = "NumUnread", default)]
    unread: i64,
    #[serde(rename = "NumAttachments", default)]
    attachments: i64,
    #[serde(rename = "Size", default)]
    size: i64,
    #[serde(rename = "Order", default)]
    order: i64,
    #[serde(
        rename = "ContextTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    context_time: Timestamp,
    #[serde(
        rename = "ExpirationTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    expiration_time: Timestamp,
}

impl Conversation {
    #[must_use]
    pub fn id(&self) -> &ConversationId {
        &self.id
    }

    #[must_use]
    pub fn subject(&self) -> &str {
        &self.subject
    }

    #[must_use]
    pub fn senders(&self) -> &[MailAddress] {
        &self.senders
    }

    #[must_use]
    pub fn recipients(&self) -> &[MailAddress] {
        &self.recipients
    }

    #[must_use]
    pub fn labels(&self) -> &[ConversationLabel] {
        &self.labels
    }

    #[must_use]
    pub fn label(&self, id: &LabelId) -> Option<&ConversationLabel> {
        self.labels.iter().find(|label| label.id() == id)
    }

    #[must_use]
    pub fn messages(&self) -> i64 {
        self.messages
    }

    #[must_use]
    pub fn unread(&self) -> i64 {
        self.unread
    }

    #[must_use]
    pub fn attachments(&self) -> i64 {
        self.attachments
    }

    #[must_use]
    pub fn size(&self) -> i64 {
        self.size
    }

    #[must_use]
    pub fn order(&self) -> i64 {
        self.order
    }

    #[must_use]
    pub fn context_time(&self) -> Timestamp {
        self.context_time
    }

    #[must_use]
    pub fn expiration_time(&self) -> Timestamp {
        self.expiration_time
    }

    #[must_use]
    pub fn is_unread(&self) -> bool {
        self.unread > 0
    }
}

#[derive(Debug, Clone, Default)]
pub struct ConversationThread {
    conversation: Conversation,
    messages: Vec<MessageMetadata>,
}

impl ConversationThread {
    #[must_use]
    pub fn new(conversation: Conversation, messages: Vec<MessageMetadata>) -> ConversationThread {
        ConversationThread {
            conversation,
            messages,
        }
    }

    #[must_use]
    pub fn conversation(&self) -> &Conversation {
        &self.conversation
    }

    #[must_use]
    pub fn messages(&self) -> &[MessageMetadata] {
        &self.messages
    }

    #[must_use]
    pub fn into_parts(self) -> (Conversation, Vec<MessageMetadata>) {
        (self.conversation, self.messages)
    }
}

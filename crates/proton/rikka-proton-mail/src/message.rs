use jiff::Timestamp;
use std::collections::BTreeMap;

use rikka_proton_account::AddressId;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::address::MailAddress;
use crate::attachment::Attachment;
use crate::flags::MessageFlags;
use crate::identifier::{AttachmentId, ConversationId, LabelId, MessageId};
use crate::mime::MimeType;
use crate::system_label::SystemLabel;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageMetadata {
    #[serde(rename = "ID", default)]
    id: MessageId,
    #[serde(rename = "ConversationID", default)]
    conversation_id: ConversationId,
    #[serde(rename = "AddressID", default)]
    address_id: AddressId,
    #[serde(rename = "Subject", default)]
    subject: String,
    #[serde(rename = "Sender", default)]
    sender: MailAddress,
    #[serde(rename = "ToList", default)]
    to: Vec<MailAddress>,
    #[serde(rename = "CCList", default)]
    cc: Vec<MailAddress>,
    #[serde(rename = "BCCList", default)]
    bcc: Vec<MailAddress>,
    #[serde(rename = "ReplyTos", default)]
    reply_tos: Vec<MailAddress>,
    #[serde(rename = "LabelIDs", default)]
    label_ids: Vec<LabelId>,
    #[serde(rename = "ExternalID", default)]
    external_id: Option<String>,
    #[serde(rename = "Flags", default)]
    flags: MessageFlags,
    #[serde(rename = "Time", default, with = "rikka_proton_api::unix_seconds")]
    time: Timestamp,
    #[serde(rename = "Size", default)]
    size: i64,
    #[serde(rename = "Order", default)]
    order: i64,
    #[serde(
        rename = "ExpirationTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    expiration_time: Timestamp,
    #[serde(rename = "NumAttachments", default)]
    attachment_count: i32,
    #[serde(rename = "Unread", default, with = "rikka_proton_api::flag")]
    unread: bool,
    #[serde(rename = "IsReplied", default, with = "rikka_proton_api::flag")]
    replied: bool,
    #[serde(rename = "IsRepliedAll", default, with = "rikka_proton_api::flag")]
    replied_all: bool,
    #[serde(rename = "IsForwarded", default, with = "rikka_proton_api::flag")]
    forwarded: bool,
}

impl MessageMetadata {
    #[must_use]
    pub fn id(&self) -> &MessageId {
        &self.id
    }

    #[must_use]
    pub fn conversation_id(&self) -> &ConversationId {
        &self.conversation_id
    }

    #[must_use]
    pub fn address_id(&self) -> &AddressId {
        &self.address_id
    }

    #[must_use]
    pub fn subject(&self) -> &str {
        &self.subject
    }

    #[must_use]
    pub fn sender(&self) -> &MailAddress {
        &self.sender
    }

    #[must_use]
    pub fn to(&self) -> &[MailAddress] {
        &self.to
    }

    #[must_use]
    pub fn cc(&self) -> &[MailAddress] {
        &self.cc
    }

    #[must_use]
    pub fn bcc(&self) -> &[MailAddress] {
        &self.bcc
    }

    #[must_use]
    pub fn reply_tos(&self) -> &[MailAddress] {
        &self.reply_tos
    }

    #[must_use]
    pub fn label_ids(&self) -> &[LabelId] {
        &self.label_ids
    }

    #[must_use]
    pub fn external_id(&self) -> Option<&str> {
        self.external_id.as_deref()
    }

    #[must_use]
    pub fn flags(&self) -> MessageFlags {
        self.flags
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
    pub fn order(&self) -> i64 {
        self.order
    }

    #[must_use]
    pub fn expiration_time(&self) -> Timestamp {
        self.expiration_time
    }

    #[must_use]
    pub fn attachment_count(&self) -> i32 {
        self.attachment_count
    }

    #[must_use]
    pub fn is_unread(&self) -> bool {
        self.unread
    }

    #[must_use]
    pub fn is_replied(&self) -> bool {
        self.replied
    }

    #[must_use]
    pub fn is_replied_all(&self) -> bool {
        self.replied_all
    }

    #[must_use]
    pub fn is_forwarded(&self) -> bool {
        self.forwarded
    }

    #[must_use]
    pub fn is_draft(&self) -> bool {
        self.flags.is_draft()
    }

    #[must_use]
    pub fn is_starred(&self) -> bool {
        self.has_label(&SystemLabel::Starred.id())
    }

    #[must_use]
    pub fn has_label(&self, label: &LabelId) -> bool {
        self.label_ids.contains(label)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Message {
    #[serde(flatten)]
    metadata: MessageMetadata,
    #[serde(rename = "Body", default)]
    body: String,
    #[serde(rename = "MIMEType", default)]
    mime_type: MimeType,
    #[serde(rename = "Header", default)]
    header: String,
    #[serde(rename = "ParsedHeaders", default)]
    parsed_headers: BTreeMap<String, Value>,
    #[serde(rename = "Attachments", default)]
    attachments: Vec<Attachment>,
}

impl Message {
    #[must_use]
    pub fn metadata(&self) -> &MessageMetadata {
        &self.metadata
    }

    #[must_use]
    pub fn id(&self) -> &MessageId {
        self.metadata.id()
    }

    #[must_use]
    pub fn address_id(&self) -> &AddressId {
        self.metadata.address_id()
    }

    #[must_use]
    pub fn subject(&self) -> &str {
        self.metadata.subject()
    }

    #[must_use]
    pub fn sender(&self) -> &MailAddress {
        self.metadata.sender()
    }

    #[must_use]
    pub fn body(&self) -> &str {
        &self.body
    }

    #[must_use]
    pub fn mime_type(&self) -> &MimeType {
        &self.mime_type
    }

    #[must_use]
    pub fn header(&self) -> &str {
        &self.header
    }

    #[must_use]
    pub fn parsed_headers(&self) -> &BTreeMap<String, Value> {
        &self.parsed_headers
    }

    #[must_use]
    pub fn attachments(&self) -> &[Attachment] {
        &self.attachments
    }

    #[must_use]
    pub fn attachment(&self, id: &AttachmentId) -> Option<&Attachment> {
        self.attachments
            .iter()
            .find(|attachment| attachment.id() == id)
    }
}

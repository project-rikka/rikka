use rikka_proton_account::{Address, AddressId, EventId, Refresh};
use rikka_proton_api::change;
use serde::Deserialize;

use crate::conversation::Conversation;
use crate::count::LabelCount;
use crate::identifier::{ConversationId, LabelId, MessageId};
use crate::label::Label;
use crate::message::MessageMetadata;

change!(MessageEvent, MessageId, message, MessageMetadata, "Message");
change!(
    ConversationEvent,
    ConversationId,
    conversation,
    Conversation,
    "Conversation"
);
change!(LabelEvent, LabelId, label, Label, "Label");
change!(AddressEvent, AddressId, address, Address, "Address");

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Event {
    #[serde(rename = "EventID", default)]
    id: EventId,
    #[serde(rename = "Refresh", default)]
    refresh: Refresh,
    #[serde(
        rename = "More",
        default,
        deserialize_with = "rikka_proton_api::flag::deserialize"
    )]
    more: bool,
    #[serde(rename = "Messages", default)]
    messages: Vec<MessageEvent>,
    #[serde(rename = "Conversations", default)]
    conversations: Vec<ConversationEvent>,
    #[serde(rename = "Labels", default)]
    labels: Vec<LabelEvent>,
    #[serde(rename = "Addresses", default)]
    addresses: Vec<AddressEvent>,
    #[serde(rename = "MessageCounts", default)]
    message_counts: Option<Vec<LabelCount>>,
    #[serde(rename = "ConversationCounts", default)]
    conversation_counts: Option<Vec<LabelCount>>,
    #[serde(rename = "UsedSpace", default)]
    used_space: Option<i64>,
}

impl Event {
    #[must_use]
    pub fn id(&self) -> &EventId {
        &self.id
    }

    #[must_use]
    pub fn refresh(&self) -> Refresh {
        self.refresh
    }

    #[must_use]
    pub fn needs_refresh(&self) -> bool {
        !self.refresh.is_none()
    }

    #[must_use]
    pub fn has_more(&self) -> bool {
        self.more
    }

    #[must_use]
    pub fn messages(&self) -> &[MessageEvent] {
        &self.messages
    }

    #[must_use]
    pub fn conversations(&self) -> &[ConversationEvent] {
        &self.conversations
    }

    #[must_use]
    pub fn labels(&self) -> &[LabelEvent] {
        &self.labels
    }

    #[must_use]
    pub fn addresses(&self) -> &[AddressEvent] {
        &self.addresses
    }

    #[must_use]
    pub fn message_counts(&self) -> Option<&[LabelCount]> {
        self.message_counts.as_deref()
    }

    #[must_use]
    pub fn conversation_counts(&self) -> Option<&[LabelCount]> {
        self.conversation_counts.as_deref()
    }

    #[must_use]
    pub fn used_space(&self) -> Option<i64> {
        self.used_space
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
            && self.conversations.is_empty()
            && self.labels.is_empty()
            && self.addresses.is_empty()
            && self.message_counts.is_none()
            && self.conversation_counts.is_none()
    }
}

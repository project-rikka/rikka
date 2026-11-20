use jiff::Timestamp;
use rikka_proton_mail::{
    Conversation, ConversationId, LabelId, MessageId, MessageMetadata, SystemLabel,
};
use serde::{Deserialize, Serialize};

use crate::person::Person;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RowId {
    Message(MessageId),
    Conversation(ConversationId),
}

impl RowId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            RowId::Message(id) => id.as_str(),
            RowId::Conversation(id) => id.as_str(),
        }
    }

    #[must_use]
    pub fn message(&self) -> Option<&MessageId> {
        match self {
            RowId::Message(id) => Some(id),
            RowId::Conversation(_) => None,
        }
    }

    #[must_use]
    pub fn conversation(&self) -> Option<&ConversationId> {
        match self {
            RowId::Conversation(id) => Some(id),
            RowId::Message(_) => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Row {
    pub id: RowId,
    pub subject: String,
    pub people: Vec<Person>,
    #[serde(with = "rikka_proton_api::unix_seconds")]
    pub time: Timestamp,
    pub unread: bool,
    pub starred: bool,
    pub attachments: bool,
    pub draft: bool,
    pub count: usize,
    pub size: i64,
    pub labels: Vec<LabelId>,
    #[serde(default)]
    pub senders: Vec<String>,
}

impl Row {
    #[must_use]
    pub fn of_message(message: &MessageMetadata, recipients: bool) -> Row {
        let people = match recipients {
            true => message.to().iter().map(Person::from_address).collect(),
            false => vec![Person::from_address(message.sender())],
        };

        Row {
            id: RowId::Message(message.id().clone()),
            subject: message.subject().to_owned(),
            people,
            time: message.time(),
            unread: message.is_unread(),
            starred: message.is_starred(),
            attachments: message.attachment_count() > 0,
            draft: message.is_draft(),
            count: 1,
            size: message.size(),
            labels: message.label_ids().to_vec(),
            senders: vec![message.sender().address().to_owned()],
        }
    }

    #[must_use]
    pub fn of_conversation(conversation: &Conversation, folder: &LabelId, recipients: bool) -> Row {
        let people: Vec<Person> = match recipients {
            true => conversation
                .recipients()
                .iter()
                .map(Person::from_address)
                .collect(),
            false => conversation
                .senders()
                .iter()
                .map(Person::from_address)
                .collect(),
        };
        let label = conversation.label(folder);
        let drafts = conversation
            .label(&SystemLabel::Drafts.id())
            .is_some_and(|label| label.messages() > 0);

        Row {
            id: RowId::Conversation(conversation.id().clone()),
            subject: conversation.subject().to_owned(),
            people,
            time: label.map_or_else(|| conversation.context_time(), |label| label.time()),
            unread: label.map_or_else(|| conversation.is_unread(), |label| label.unread() > 0),
            starred: conversation.label(&SystemLabel::Starred.id()).is_some(),
            attachments: conversation.attachments() > 0,
            draft: drafts,
            count: usize::try_from(
                label.map_or_else(|| conversation.messages(), |label| label.messages()),
            )
            .unwrap_or(1),
            size: conversation.size(),
            labels: conversation
                .labels()
                .iter()
                .map(|label| label.id().clone())
                .collect(),
            senders: conversation
                .senders()
                .iter()
                .map(|sender| sender.address().to_owned())
                .collect(),
        }
    }

    #[must_use]
    pub fn named(&self) -> &str {
        match self.subject.trim().is_empty() {
            true => "",
            false => self.subject.as_str(),
        }
    }

    #[must_use]
    pub fn party(&self) -> String {
        let mut names = String::new();
        for person in &self.people {
            if !names.is_empty() {
                names.push_str(", ");
            }
            names.push_str(person.shown());
        }
        names
    }

    #[must_use]
    pub fn has_label(&self, label: &LabelId) -> bool {
        self.labels.contains(label)
    }
}

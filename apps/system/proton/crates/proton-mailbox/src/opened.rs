use jiff::Timestamp;
use rikka_proton_mail::{
    Attachment, AttachmentId, DecryptedMessage, Disposition, LabelId, Message, MessageId,
};
use rikka_proton_pgp::SignatureStatus;

use crate::body::{Body, decode};
use crate::person::Person;
use crate::row::RowId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verification {
    Verified,
    Unsigned,
    Unknown,
    Broken,
}

impl Verification {
    #[must_use]
    pub fn from_status(status: &SignatureStatus) -> Verification {
        match status {
            SignatureStatus::Valid { .. } => Verification::Verified,
            SignatureStatus::Missing => Verification::Unsigned,
            SignatureStatus::UnknownKey { .. } => Verification::Unknown,
            _ => Verification::Broken,
        }
    }
}

#[derive(Clone, Debug)]
pub struct MessageAttachment {
    pub id: AttachmentId,
    pub name: String,
    pub media_type: String,
    pub size: i64,
    pub inline: bool,
    pub source: Attachment,
}

impl MessageAttachment {
    #[must_use]
    pub fn from_attachment(attachment: &Attachment) -> MessageAttachment {
        MessageAttachment {
            id: attachment.id().clone(),
            name: attachment.name().to_owned(),
            media_type: attachment.mime_type().to_owned(),
            size: attachment.size(),
            inline: matches!(attachment.disposition(), Disposition::Inline),
            source: attachment.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct OpenedMessage {
    pub id: MessageId,
    pub address: rikka_proton_account::AddressId,
    pub subject: String,
    pub sender: Person,
    pub to: Vec<Person>,
    pub cc: Vec<Person>,
    pub bcc: Vec<Person>,
    pub reply_to: Vec<Person>,
    pub time: Timestamp,
    pub unread: bool,
    pub starred: bool,
    pub draft: bool,
    pub words: Body,
    pub signature: Verification,
    pub files: Vec<MessageAttachment>,
    pub labels: Vec<LabelId>,
    pub external: Option<String>,
}

impl OpenedMessage {
    #[must_use]
    pub fn from_message(message: &Message, decrypted: &DecryptedMessage) -> OpenedMessage {
        let metadata = message.metadata();
        OpenedMessage {
            id: metadata.id().clone(),
            address: metadata.address_id().clone(),
            subject: metadata.subject().to_owned(),
            sender: Person::from_address(metadata.sender()),
            to: metadata.to().iter().map(Person::from_address).collect(),
            cc: metadata.cc().iter().map(Person::from_address).collect(),
            bcc: metadata.bcc().iter().map(Person::from_address).collect(),
            reply_to: metadata
                .reply_tos()
                .iter()
                .map(Person::from_address)
                .collect(),
            time: metadata.time(),
            unread: metadata.is_unread(),
            starred: metadata.is_starred(),
            draft: metadata.is_draft(),
            words: decode(decrypted.mime_type(), decrypted.bytes()),
            signature: Verification::from_status(decrypted.signature()),
            files: message
                .attachments()
                .iter()
                .map(MessageAttachment::from_attachment)
                .collect(),
            labels: metadata.label_ids().to_vec(),
            external: metadata.external_id().map(str::to_owned),
        }
    }

    #[must_use]
    pub fn answering(&self) -> Vec<Person> {
        match self.reply_to.is_empty() {
            true => vec![self.sender.clone()],
            false => self.reply_to.clone(),
        }
    }

    #[must_use]
    pub fn shown_files(&self) -> Vec<&MessageAttachment> {
        self.files.iter().filter(|file| !file.inline).collect()
    }
}

#[derive(Clone, Debug)]
pub struct OpenedThread {
    pub id: RowId,
    pub subject: String,
    pub messages: Vec<OpenedMessage>,
    pub thread: i64,
}

impl OpenedThread {
    #[must_use]
    pub fn newest(&self) -> Option<&OpenedMessage> {
        self.messages.last()
    }

    #[must_use]
    pub fn message(&self, id: &MessageId) -> Option<&OpenedMessage> {
        self.messages.iter().find(|message| &message.id == id)
    }

    #[must_use]
    pub fn is_unread(&self) -> bool {
        self.messages.iter().any(|message| message.unread)
    }

    #[must_use]
    pub fn is_starred(&self) -> bool {
        self.messages.iter().any(|message| message.starred)
    }
}

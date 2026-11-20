use std::fmt;

use rikka_proton_pgp::{Plaintext, SignatureStatus};

use crate::attachment::Attachment;
use crate::identifier::{AttachmentId, MessageId};
use crate::message::Message;
use crate::mime::{MimeContent, MimeMessage, MimeType, part_signature};

#[derive(Clone)]
pub struct DecryptedMessage {
    id: MessageId,
    mime_type: MimeType,
    body: Vec<u8>,
    signature: SignatureStatus,
    mime: Option<MimeMessage>,
}

impl DecryptedMessage {
    pub(crate) fn new(
        message: &Message,
        plaintext: Plaintext,
        mime: Option<MimeMessage>,
    ) -> DecryptedMessage {
        let signature = effective_signature(plaintext.signature(), mime.as_ref());

        DecryptedMessage {
            id: message.id().clone(),
            mime_type: message.mime_type().clone(),
            signature,
            body: plaintext.into_data(),
            mime,
        }
    }

    #[must_use]
    pub fn id(&self) -> &MessageId {
        &self.id
    }

    #[must_use]
    pub fn mime_type(&self) -> &MimeType {
        &self.mime_type
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.body
    }

    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.body
    }

    #[must_use]
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    #[must_use]
    pub fn mime(&self) -> Option<&MimeMessage> {
        self.mime.as_ref()
    }

    #[must_use]
    pub fn content(&self) -> MimeContent {
        message_content(&self.mime_type, &self.body, self.mime.as_ref())
    }

    #[must_use]
    pub fn signature(&self) -> &SignatureStatus {
        &self.signature
    }

    #[must_use]
    pub fn is_verified(&self) -> bool {
        self.signature.is_valid()
    }
}

pub(crate) fn message_content(
    mime_type: &MimeType,
    body: &[u8],
    mime: Option<&MimeMessage>,
) -> MimeContent {
    match (mime, mime_type) {
        (Some(mime), _) => mime.content(),
        (None, MimeType::Html) => {
            MimeContent::from_html(String::from_utf8_lossy(body).into_owned())
        }
        (None, _) => MimeContent::from_text(String::from_utf8_lossy(body).into_owned()),
    }
}

pub(crate) fn effective_signature(
    packet: &SignatureStatus,
    mime: Option<&MimeMessage>,
) -> SignatureStatus {
    match mime.and_then(|mime| part_signature(mime.root())) {
        Some(inner) if packet.is_missing() => inner.clone(),
        _ => packet.clone(),
    }
}

impl fmt::Debug for DecryptedMessage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DecryptedMessage")
            .field("id", &self.id)
            .field("mime_type", &self.mime_type)
            .field("bytes", &self.body.len())
            .field("signature", &self.signature)
            .field("mime", &self.mime.is_some())
            .finish()
    }
}

#[derive(Clone)]
pub struct DecryptedAttachment {
    id: AttachmentId,
    name: String,
    mime_type: String,
    data: Vec<u8>,
    signature: SignatureStatus,
}

impl DecryptedAttachment {
    pub(crate) fn new(
        attachment: &Attachment,
        plaintext: Plaintext,
        signature: SignatureStatus,
    ) -> DecryptedAttachment {
        DecryptedAttachment {
            id: attachment.id().clone(),
            name: attachment.name().to_owned(),
            mime_type: attachment.mime_type().to_owned(),
            signature,
            data: plaintext.into_data(),
        }
    }

    #[must_use]
    pub fn id(&self) -> &AttachmentId {
        &self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn mime_type(&self) -> &str {
        &self.mime_type
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.data
    }

    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.data
    }

    #[must_use]
    pub fn signature(&self) -> &SignatureStatus {
        &self.signature
    }

    #[must_use]
    pub fn is_verified(&self) -> bool {
        self.signature.is_valid()
    }
}

impl fmt::Debug for DecryptedAttachment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DecryptedAttachment")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("mime_type", &self.mime_type)
            .field("bytes", &self.data.len())
            .field("signature", &self.signature)
            .finish()
    }
}

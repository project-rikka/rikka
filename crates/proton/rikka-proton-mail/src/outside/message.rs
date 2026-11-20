use jiff::Timestamp;
use std::fmt;

use rikka_proton_pgp::{Pgp, PublicKey, SignatureStatus};
use serde::Deserialize;

use crate::address::MailAddress;
use crate::attachment::Attachment;
use crate::decrypted::{effective_signature, message_content};
use crate::error::Result;
use crate::mime::{MimeContent, MimeDecryptor, MimeMessage, MimeType, Secret};
use crate::outside::password_decryption_error;

#[derive(Debug, Deserialize)]
pub(crate) struct EncryptedOutsideMessage {
    #[serde(rename = "Subject", default)]
    subject: String,
    #[serde(rename = "Sender", default)]
    sender: MailAddress,
    #[serde(rename = "ToList", default)]
    to: Vec<MailAddress>,
    #[serde(rename = "CCList", default)]
    cc: Vec<MailAddress>,
    #[serde(rename = "Time", default, with = "rikka_proton_api::unix_seconds")]
    time: Timestamp,
    #[serde(
        rename = "ExpirationTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    expiration_time: Timestamp,
    #[serde(rename = "NumAttachments", default)]
    attachment_count: i32,
    #[serde(rename = "Body", default)]
    body: String,
    #[serde(rename = "MIMEType", default)]
    mime_type: MimeType,
    #[serde(rename = "Recipient", default)]
    recipient: String,
    #[serde(rename = "Attachments", default)]
    attachments: Vec<Attachment>,
    #[serde(rename = "Replies", default)]
    replies: Vec<EncryptedOutsideReply>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EncryptedOutsideReply {
    #[serde(rename = "Body", default)]
    body: String,
    #[serde(rename = "Time", default, with = "rikka_proton_api::unix_seconds")]
    time: Timestamp,
    #[serde(rename = "Attachments", default)]
    attachments: Vec<Attachment>,
}

impl EncryptedOutsideReply {
    pub(crate) fn decrypt(self, pgp: &Pgp, password: &[u8]) -> Result<SentReply> {
        let body = match self.body.is_empty() {
            true => String::new(),
            false => {
                let plaintext = pgp
                    .decrypt()
                    .with_password(password)
                    .message(self.body.as_bytes())
                    .map_err(password_decryption_error)?;
                String::from_utf8_lossy(plaintext.data()).into_owned()
            }
        };

        Ok(SentReply {
            body,
            time: self.time,
            attachments: self.attachments,
        })
    }
}

#[derive(Debug, Clone)]
pub struct SentReply {
    body: String,
    time: Timestamp,
    attachments: Vec<Attachment>,
}

impl SentReply {
    #[must_use]
    pub fn body(&self) -> &str {
        &self.body
    }

    #[must_use]
    pub fn time(&self) -> Timestamp {
        self.time
    }

    #[must_use]
    pub fn attachments(&self) -> &[Attachment] {
        &self.attachments
    }
}

#[derive(Clone)]
pub struct OutsideMessage {
    subject: String,
    sender: MailAddress,
    to: Vec<MailAddress>,
    cc: Vec<MailAddress>,
    time: Timestamp,
    expiration_time: Timestamp,
    attachment_count: i32,
    mime_type: MimeType,
    recipient: String,
    attachments: Vec<Attachment>,
    replies: Vec<SentReply>,
    sender_key: Option<PublicKey>,
    body: Vec<u8>,
    signature: SignatureStatus,
    mime: Option<MimeMessage>,
}

impl OutsideMessage {
    pub(crate) fn decrypt(
        pgp: &Pgp,
        password: &[u8],
        encrypted: EncryptedOutsideMessage,
        sender_key: Option<PublicKey>,
    ) -> Result<OutsideMessage> {
        let verifiers: Vec<PublicKey> = sender_key.iter().cloned().collect();

        let plaintext = pgp
            .decrypt()
            .with_password(password)
            .verified_by_keys(verifiers.clone())
            .message(encrypted.body.as_bytes())
            .map_err(password_decryption_error)?;

        let mime = encrypted.mime_type.is_mime().then(|| {
            MimeDecryptor::new(pgp, Secret::Password(password), &verifiers).parse(plaintext.data())
        });
        let signature = effective_signature(plaintext.signature(), mime.as_ref());

        let replies = encrypted
            .replies
            .into_iter()
            .map(|reply| reply.decrypt(pgp, password))
            .collect::<Result<Vec<_>>>()?;

        Ok(OutsideMessage {
            subject: encrypted.subject,
            sender: encrypted.sender,
            to: encrypted.to,
            cc: encrypted.cc,
            time: encrypted.time,
            expiration_time: encrypted.expiration_time,
            attachment_count: encrypted.attachment_count,
            mime_type: encrypted.mime_type,
            recipient: encrypted.recipient,
            attachments: encrypted.attachments,
            replies,
            sender_key,
            body: plaintext.into_data(),
            signature,
            mime,
        })
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
    pub fn time(&self) -> Timestamp {
        self.time
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
    pub fn mime_type(&self) -> &MimeType {
        &self.mime_type
    }

    #[must_use]
    pub fn recipient(&self) -> &str {
        &self.recipient
    }

    #[must_use]
    pub fn attachments(&self) -> &[Attachment] {
        &self.attachments
    }

    #[must_use]
    pub fn replies(&self) -> &[SentReply] {
        &self.replies
    }

    #[must_use]
    pub fn sender_key(&self) -> Option<&PublicKey> {
        self.sender_key.as_ref()
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.body
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

impl fmt::Debug for OutsideMessage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OutsideMessage")
            .field("subject", &self.subject)
            .field("sender", &self.sender)
            .field("recipient", &self.recipient)
            .field("mime_type", &self.mime_type)
            .field("bytes", &self.body.len())
            .field("attachments", &self.attachments.len())
            .field("replies", &self.replies.len())
            .field("signature", &self.signature)
            .finish_non_exhaustive()
    }
}

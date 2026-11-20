use jiff::Timestamp;
use std::fmt;

use rikka_proton_account::{Address, AddressId};
use rikka_proton_api::offload;
use rikka_proton_pgp::{Pgp, PrivateKey, SessionKey};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::address::MailAddress;
use crate::client::{MESSAGES, MailClient};
use crate::error::{Error, Result};
use crate::identifier::{AttachmentId, MessageId};
use crate::message::Message;
use crate::mime::{AttachmentFile, MimeType, OutgoingFile, mime_body};
use crate::recipients::{Bodies, Delivery, Recipients};
use crate::send_options::{PUBLIC_KEY_TYPE, SendOptions, public_key_file};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DirectAction {
    Reply,
    ReplyAll,
    Forward,
    AutoResponse,
    ReadReceipt,
}

impl DirectAction {
    #[must_use]
    pub fn number(self) -> i32 {
        match self {
            DirectAction::Reply => 0,
            DirectAction::ReplyAll => 1,
            DirectAction::Forward => 2,
            DirectAction::AutoResponse => 3,
            DirectAction::ReadReceipt => 4,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DirectSent {
    delivery_time: Timestamp,
    sent: Option<Message>,
}

impl DirectSent {
    #[must_use]
    pub fn delivery_time(&self) -> Timestamp {
        self.delivery_time
    }

    #[must_use]
    pub fn sent(&self) -> Option<&Message> {
        self.sent.as_ref()
    }
}

#[must_use]
#[derive(Clone)]
pub struct DirectSend {
    client: MailClient,
    sender: Option<AddressId>,
    subject: String,
    to: Vec<MailAddress>,
    cc: Vec<MailAddress>,
    bcc: Vec<MailAddress>,
    body: String,
    mime_type: Option<MimeType>,
    options: SendOptions,
    files: Vec<AttachmentFile>,
    parent: Option<(MessageId, DirectAction)>,
    auto_save_contacts: bool,
    expires_in: Option<i64>,
}

impl DirectSend {
    pub(crate) fn new(client: MailClient) -> DirectSend {
        DirectSend {
            client,
            sender: None,
            subject: String::new(),
            to: Vec::new(),
            cc: Vec::new(),
            bcc: Vec::new(),
            body: String::new(),
            mime_type: None,
            options: SendOptions::default(),
            files: Vec::new(),
            parent: None,
            auto_save_contacts: false,
            expires_in: None,
        }
    }

    pub fn sender(mut self, address: &AddressId) -> Self {
        self.sender = Some(address.clone());
        self
    }

    pub fn subject(mut self, subject: impl Into<String>) -> Self {
        self.subject = subject.into();
        self
    }

    pub fn to(mut self, recipient: impl Into<MailAddress>) -> Self {
        self.to.push(recipient.into());
        self
    }

    pub fn cc(mut self, recipient: impl Into<MailAddress>) -> Self {
        self.cc.push(recipient.into());
        self
    }

    pub fn bcc(mut self, recipient: impl Into<MailAddress>) -> Self {
        self.bcc.push(recipient.into());
        self
    }

    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = body.into();
        self
    }

    pub fn mime_type(mut self, mime_type: MimeType) -> Self {
        self.mime_type = Some(mime_type);
        self
    }

    pub fn sign(mut self, sign: bool) -> Self {
        self.options.sign = Some(sign);
        self
    }

    pub fn attach_public_key(mut self, attach: bool) -> Self {
        self.options.attach_public_key = Some(attach);
        self
    }

    pub fn attach(
        mut self,
        name: impl Into<String>,
        mime_type: impl Into<String>,
        data: impl Into<Vec<u8>>,
    ) -> Self {
        self.files.push(AttachmentFile {
            name: name.into(),
            mime_type: mime_type.into(),
            content_id: None,
            data: data.into(),
        });
        self
    }

    pub fn attach_inline(
        self,
        name: impl Into<String>,
        mime_type: impl Into<String>,
        content_id: impl Into<String>,
        data: impl Into<Vec<u8>>,
    ) -> Self {
        let mut attached = self.attach(name, mime_type, data);
        if let Some(last) = attached.files.last_mut() {
            last.content_id = Some(content_id.into());
        }
        attached
    }

    pub fn in_reply_to(mut self, parent: &MessageId, action: DirectAction) -> Self {
        self.parent = Some((parent.clone(), action));
        self
    }

    pub fn auto_save_contacts(mut self, save: bool) -> Self {
        self.auto_save_contacts = save;
        self
    }

    pub fn expires_in(mut self, lifetime: std::time::Duration) -> Self {
        self.expires_in = Some(i64::try_from(lifetime.as_secs()).unwrap_or(i64::MAX));
        self
    }

    fn unresolved_recipients(&self) -> Result<Recipients> {
        let mut recipients = Recipients::default();
        for recipient in self.to.iter().chain(&self.cc).chain(&self.bcc) {
            recipients.add(recipient.address(), Delivery::Resolve);
        }
        if recipients.is_empty() {
            return Err(Error::NoRecipient);
        }
        Ok(recipients)
    }

    fn attach_sender_key(&mut self, sender: &Address, key: &PrivateKey) -> Result<()> {
        let file = public_key_file(sender, key)?;
        if !self.files.iter().any(|known| known.name == file.name) {
            self.files.push(AttachmentFile {
                name: file.name,
                mime_type: PUBLIC_KEY_TYPE.to_owned(),
                content_id: None,
                data: file.armored.into_bytes(),
            });
        }
        Ok(())
    }

    fn mime_body(&self, mime_type: &MimeType) -> String {
        let files: Vec<OutgoingFile<'_>> =
            self.files.iter().map(AttachmentFile::as_outgoing).collect();
        mime_body(&self.body, *mime_type == MimeType::Html, &files)
    }

    pub async fn send(mut self) -> Result<DirectSent> {
        let address = self.sender.clone().ok_or(Error::MissingSender)?;
        let (sender_address, key) = self.client.sending_key(&address)?;

        let mut recipients = self.unresolved_recipients()?;
        let settings = self.client.cached_mail_settings().await?;
        let decision = self.options.decide(&settings, self.mime_type.as_ref());
        recipients.resolve(&self.client, &decision).await?;

        if decision.attach_public_key {
            self.attach_sender_key(&sender_address, &key)?;
        }

        let mime_type = decision.mime_type.clone();
        let mime = recipients
            .wants_mime(&mime_type)
            .then(|| self.mime_body(&mime_type));

        let sender = MailAddress::with_name(
            sender_address.email(),
            sender_address.display_name().unwrap_or_default(),
        );
        let client = self.client.clone();
        let sealing = self.clone();

        let (message, packages) = offload(move || -> Result<(Value, Vec<Value>)> {
            let pgp = client.pgp();
            let body = pgp
                .encrypt()
                .to(key.public_key())
                .signed_by(key.clone())
                .as_utf8()
                .armored_message(&sealing.body)?;

            let EncryptedAttachments {
                files: attachments,
                session_keys: keys,
            } = encrypt_attachments(pgp, &key, &sealing.files)?;

            let order: Vec<AttachmentId> = keys.iter().map(|(id, _)| id.clone()).collect();
            let packages = recipients
                .packages(
                    &client,
                    &key,
                    &Bodies {
                        text: &sealing.body,
                        mime_type: &mime_type,
                        mime: mime.as_deref(),
                    },
                    &keys,
                )?
                .iter()
                .map(|package| package.to_ordered_json(&order))
                .collect();

            let message = serde_json::to_value(DirectMessage {
                subject: &sealing.subject,
                sender: &sender,
                to: &sealing.to,
                cc: &sealing.cc,
                bcc: &sealing.bcc,
                body,
                mime_type: &mime_type,
                attachments,
            })
            .map_err(|source| Error::Encoding {
                what: "direct message",
                source,
            })?;

            Ok((message, packages))
        })
        .await?;

        let request = DirectRequest {
            message,
            parent: self.parent.as_ref().map(|(parent, _)| parent),
            action: self.parent.as_ref().map(|(_, action)| action.number()),
            expires_in: self.expires_in,
            auto_save_contacts: u8::from(self.auto_save_contacts),
            packages,
        };

        let response: DirectResponse = self
            .client
            .api()
            .post(format!("{MESSAGES}/send/direct"))
            .json(&request)
            .send()
            .await?;

        Ok(DirectSent {
            delivery_time: response.delivery_time,
            sent: response.sent,
        })
    }
}

impl fmt::Debug for DirectSend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DirectSend")
            .field("sender", &self.sender)
            .field(
                "recipients",
                &(self.to.len() + self.cc.len() + self.bcc.len()),
            )
            .field("mime_type", &self.mime_type)
            .field("body", &self.body.len())
            .field("files", &self.files.len())
            .finish_non_exhaustive()
    }
}

#[derive(Serialize)]
struct DirectFile<'a> {
    #[serde(rename = "Filename")]
    name: &'a str,
    #[serde(rename = "MIMEType")]
    mime_type: &'a str,
    #[serde(rename = "ContentID", skip_serializing_if = "Option::is_none")]
    content_id: Option<&'a str>,
    #[serde(rename = "Contents")]
    contents: String,
}

#[derive(Serialize)]
struct DirectMessage<'a> {
    #[serde(rename = "Subject")]
    subject: &'a str,
    #[serde(rename = "Sender")]
    sender: &'a MailAddress,
    #[serde(rename = "ToList")]
    to: &'a [MailAddress],
    #[serde(rename = "CCList")]
    cc: &'a [MailAddress],
    #[serde(rename = "BCCList")]
    bcc: &'a [MailAddress],
    #[serde(rename = "Body")]
    body: String,
    #[serde(rename = "MIMEType")]
    mime_type: &'a MimeType,
    #[serde(rename = "Attachments")]
    attachments: Vec<DirectFile<'a>>,
}

#[derive(Serialize)]
struct DirectRequest<'a> {
    #[serde(rename = "Message")]
    message: Value,
    #[serde(rename = "ParentID", skip_serializing_if = "Option::is_none")]
    parent: Option<&'a MessageId>,
    #[serde(rename = "Action", skip_serializing_if = "Option::is_none")]
    action: Option<i32>,
    #[serde(rename = "ExpiresIn", skip_serializing_if = "Option::is_none")]
    expires_in: Option<i64>,
    #[serde(rename = "AutoSaveContacts")]
    auto_save_contacts: u8,
    #[serde(rename = "Packages")]
    packages: Vec<Value>,
}

#[derive(Deserialize)]
struct DirectResponse {
    #[serde(
        rename = "DeliveryTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    delivery_time: Timestamp,
    #[serde(rename = "Sent", default)]
    sent: Option<Message>,
}

struct EncryptedAttachments<'a> {
    files: Vec<DirectFile<'a>>,
    session_keys: Vec<(AttachmentId, SessionKey)>,
}

fn encrypt_attachments<'a>(
    pgp: &Pgp,
    key: &PrivateKey,
    files: &'a [AttachmentFile],
) -> Result<EncryptedAttachments<'a>> {
    let mut attachments = Vec::with_capacity(files.len());
    let mut keys = Vec::with_capacity(files.len());
    for (index, file) in files.iter().enumerate() {
        let (key_packets, data_packet, session_key) = pgp
            .encrypt()
            .to(key.public_key())
            .split(&file.data)?
            .into_parts();
        let signature = pgp.sign().with_key(key.clone()).detached(&file.data)?;

        let mut contents = key_packets;
        contents.extend_from_slice(&data_packet);
        contents.extend_from_slice(&signature);

        attachments.push(DirectFile {
            name: &file.name,
            mime_type: &file.mime_type,
            content_id: file.content_id.as_deref(),
            contents: rikka_proton_api::base64::encode(contents),
        });
        keys.push((AttachmentId::from(index.to_string()), session_key));
    }
    Ok(EncryptedAttachments {
        files: attachments,
        session_keys: keys,
    })
}

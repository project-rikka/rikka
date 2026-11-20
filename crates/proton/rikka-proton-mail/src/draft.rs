use std::collections::BTreeMap;
use std::fmt;

use rikka_proton_account::AddressId;
use serde::Serialize;

use crate::address::MailAddress;
use crate::client::{MESSAGES, MailClient};
use crate::error::{Error, Result};
use crate::flags::MessageFlags;
use crate::identifier::{AttachmentId, MessageId};
use crate::message::Message;
use crate::mime::MimeType;
use crate::settings::MailSettings;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DraftAction {
    Reply,
    ReplyAll,
    Forward,
}

impl DraftAction {
    #[must_use]
    pub fn number(self) -> i32 {
        match self {
            DraftAction::Reply => 0,
            DraftAction::ReplyAll => 1,
            DraftAction::Forward => 2,
        }
    }
}

impl Serialize for DraftAction {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_i32(self.number())
    }
}

#[must_use]
#[derive(Clone)]
pub struct DraftBuilder {
    client: MailClient,
    sender: Option<AddressId>,
    subject: String,
    to: Vec<MailAddress>,
    cc: Vec<MailAddress>,
    bcc: Vec<MailAddress>,
    body: String,
    mime_type: Option<MimeType>,
    unread: bool,
    external_id: Option<String>,
    flags: MessageFlags,
    parent: Option<(MessageId, DraftAction)>,
    attachment_key_packets: BTreeMap<AttachmentId, String>,
}

impl DraftBuilder {
    pub(crate) fn new(client: MailClient) -> DraftBuilder {
        DraftBuilder {
            client,
            sender: None,
            subject: String::new(),
            to: Vec::new(),
            cc: Vec::new(),
            bcc: Vec::new(),
            body: String::new(),
            mime_type: None,
            unread: false,
            external_id: None,
            flags: MessageFlags::empty(),
            parent: None,
            attachment_key_packets: BTreeMap::new(),
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

    pub fn unread(mut self, unread: bool) -> Self {
        self.unread = unread;
        self
    }

    pub fn external_id(mut self, id: impl Into<String>) -> Self {
        self.external_id = Some(id.into());
        self
    }

    pub fn flags(mut self, flags: MessageFlags) -> Self {
        self.flags = flags;
        self
    }

    pub fn in_reply_to(mut self, parent: &MessageId, action: DraftAction) -> Self {
        self.parent = Some((parent.clone(), action));
        self
    }

    pub fn attachment_key_packet(mut self, id: &AttachmentId, packet: impl Into<String>) -> Self {
        self.attachment_key_packets
            .insert(id.clone(), packet.into());
        self
    }

    pub async fn create(self) -> Result<Message> {
        let (parent, action) = self.parent.clone().unzip();
        let settings = self.client.cached_mail_settings().await?;
        let mut template = self.template(&settings)?;
        if settings.attaches_public_key() {
            template.flags |= MessageFlags::PUBLIC_KEY;
        }

        let request = DraftRequest {
            message: template,
            attachment_key_packets: self.attachment_key_packets.clone(),
            parent,
            action,
        };

        Ok(self
            .client
            .api()
            .post(MESSAGES)
            .json(&request)
            .send_field("Message")
            .await?)
    }

    pub async fn update(self, id: &MessageId) -> Result<Message> {
        let settings = self.client.cached_mail_settings().await?;
        let request = DraftRequest {
            message: self.template(&settings)?,
            attachment_key_packets: self.attachment_key_packets.clone(),
            parent: None,
            action: None,
        };

        Ok(self
            .client
            .api()
            .put(format!("{MESSAGES}/{id}"))
            .json(&request)
            .send_field("Message")
            .await?)
    }

    fn template(&self, settings: &MailSettings) -> Result<DraftTemplate> {
        let address = self.sender.as_ref().ok_or(Error::MissingSender)?;
        let (sender_address, key) = self.client.sending_key(address)?;

        let body = self
            .client
            .pgp()
            .encrypt()
            .to(key.public_key())
            .signed_by(key)
            .as_utf8()
            .armored_message(&self.body)?;

        Ok(DraftTemplate {
            subject: self.subject.clone(),
            sender: MailAddress::with_name(
                sender_address.email(),
                sender_address.display_name().unwrap_or_default(),
            ),
            to: self.to.clone(),
            cc: self.cc.clone(),
            bcc: self.bcc.clone(),
            body,
            mime_type: self
                .mime_type
                .clone()
                .unwrap_or_else(|| settings.draft_mime_type().clone()),
            unread: self.unread,
            external_id: self.external_id.clone(),
            flags: self.flags,
        })
    }
}

impl fmt::Debug for DraftBuilder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DraftBuilder")
            .field("sender", &self.sender)
            .field(
                "recipients",
                &(self.to.len() + self.cc.len() + self.bcc.len()),
            )
            .field("mime_type", &self.mime_type)
            .field("body", &self.body.len())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Serialize)]
struct DraftTemplate {
    #[serde(rename = "Subject")]
    subject: String,
    #[serde(rename = "Sender")]
    sender: MailAddress,
    #[serde(rename = "ToList")]
    to: Vec<MailAddress>,
    #[serde(rename = "CCList")]
    cc: Vec<MailAddress>,
    #[serde(rename = "BCCList")]
    bcc: Vec<MailAddress>,
    #[serde(rename = "Body")]
    body: String,
    #[serde(rename = "MIMEType")]
    mime_type: MimeType,
    #[serde(
        rename = "Unread",
        serialize_with = "rikka_proton_api::flag::serialize"
    )]
    unread: bool,
    #[serde(rename = "ExternalID", skip_serializing_if = "Option::is_none")]
    external_id: Option<String>,
    #[serde(rename = "Flags")]
    flags: MessageFlags,
}

#[derive(Debug, Serialize)]
struct DraftRequest {
    #[serde(rename = "Message")]
    message: DraftTemplate,
    #[serde(rename = "AttachmentKeyPackets")]
    attachment_key_packets: BTreeMap<AttachmentId, String>,
    #[serde(rename = "ParentID", skip_serializing_if = "Option::is_none")]
    parent: Option<MessageId>,
    #[serde(rename = "Action", skip_serializing_if = "Option::is_none")]
    action: Option<DraftAction>,
}

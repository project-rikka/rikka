use jiff::Timestamp;
use std::fmt;
use std::time::Duration;

use futures_util::{StreamExt, TryStreamExt, stream};
use rikka_proton_account::AddressId;
use rikka_proton_pgp::{PublicKey, SessionKey};
use serde::Serialize;

use crate::client::{MESSAGES, MailClient};
use crate::error::{Error, Result};
use crate::identifier::{AttachmentId, MessageId};
use crate::message::Message;
use crate::mime::{AttachmentFile, MimeType, OutgoingFile, mime_body};
use crate::package::Package;
use crate::recipients::{Bodies, Delivery, Recipients};
use crate::send_options::{PUBLIC_KEY_TYPE, SendOptions, public_key_file};

const ATTACHMENTS_AT_ONCE: usize = 4;

#[must_use]
#[derive(Clone)]
pub struct SendBuilder {
    client: MailClient,
    message: MessageId,
    sender: Option<AddressId>,
    body: String,
    mime_type: Option<MimeType>,
    mime_body: Option<String>,
    recipients: Recipients,
    attachment_keys: Vec<(AttachmentId, SessionKey)>,
    options: SendOptions,
    auto_save_contacts: Option<bool>,
    delay_seconds: Option<u64>,
    delivery_time: Option<Timestamp>,
    expires_in: Option<i64>,
    expiration_time: Option<Timestamp>,
}

impl SendBuilder {
    pub(crate) fn new(client: MailClient, message: MessageId) -> SendBuilder {
        SendBuilder {
            client,
            message,
            sender: None,
            body: String::new(),
            mime_type: None,
            mime_body: None,
            recipients: Recipients::default(),
            attachment_keys: Vec::new(),
            options: SendOptions::default(),
            auto_save_contacts: None,
            delay_seconds: None,
            delivery_time: None,
            expires_in: None,
            expiration_time: None,
        }
    }

    pub fn sender(mut self, address: &AddressId) -> Self {
        self.sender = Some(address.clone());
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

    pub fn mime_body(mut self, body: impl Into<String>) -> Self {
        self.mime_body = Some(body.into());
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

    pub fn to(self, email: impl Into<String>) -> Self {
        self.recipient(email, Delivery::Resolve)
    }

    pub fn to_internal(self, email: impl Into<String>, key: PublicKey) -> Self {
        self.recipient(email, Delivery::Internal(key))
    }

    pub fn to_cleartext(self, email: impl Into<String>) -> Self {
        self.recipient(email, Delivery::Cleartext { signed: false })
    }

    pub fn to_pgp_inline(self, email: impl Into<String>, key: PublicKey) -> Self {
        self.recipient(email, Delivery::PgpInline(key))
    }

    pub fn to_pgp_mime(self, email: impl Into<String>, key: PublicKey) -> Self {
        self.recipient(email, Delivery::PgpMime(key))
    }

    pub fn to_clear_mime(self, email: impl Into<String>) -> Self {
        self.recipient(email, Delivery::ClearMime)
    }

    pub fn to_password(self, email: impl Into<String>, password: impl Into<String>) -> Self {
        self.recipient(
            email,
            Delivery::Outside {
                password: password.into(),
                challenge: None,
            },
        )
    }

    pub fn password_hint(mut self, hint: impl Into<String>) -> Self {
        self.recipients.hint(hint);
        self
    }

    pub fn attachment_key(mut self, id: &AttachmentId, session_key: SessionKey) -> Self {
        self.attachment_keys.push((id.clone(), session_key));
        self
    }

    pub fn attachment_keys(
        mut self,
        keys: impl IntoIterator<Item = (AttachmentId, SessionKey)>,
    ) -> Self {
        self.attachment_keys.extend(keys);
        self
    }

    pub fn auto_save_contacts(mut self, save: bool) -> Self {
        self.auto_save_contacts = Some(save);
        self
    }

    pub fn delay(mut self, delay: Duration) -> Self {
        self.delay_seconds = Some(delay.as_secs());
        self
    }

    pub fn deliver_at(mut self, at: Timestamp) -> Self {
        self.delivery_time = Some(at);
        self
    }

    pub fn expires_in(mut self, lifetime: Duration) -> Self {
        self.expires_in = Some(i64::try_from(lifetime.as_secs()).unwrap_or(i64::MAX));
        self
    }

    pub fn expires_at(mut self, at: Timestamp) -> Self {
        self.expiration_time = Some(at);
        self
    }

    pub fn packages(&self) -> Result<Vec<Package>> {
        let mime_type = self.mime_type.clone().unwrap_or_default();
        self.packed(&mime_type, self.mime_body.as_deref(), &self.attachment_keys)
    }

    pub async fn send(mut self) -> Result<Message> {
        let address = self.sender.clone().ok_or(Error::MissingSender)?;
        let settings = self.client.cached_mail_settings().await?;
        let decision = self.options.decide(&settings, self.mime_type.as_ref());
        self.recipients.resolve(&self.client, &decision).await?;

        let wants_mime =
            self.mime_body.is_none() && self.recipients.wants_mime(&decision.mime_type);
        let mut keys = self.attachment_keys.clone();
        let mut mime = self.mime_body.clone();

        if decision.attach_public_key || wants_mime {
            let mut collected = self.load_draft_attachments(wants_mime).await?;
            if decision.attach_public_key {
                self.attach_key(&address, &mut collected, &mut keys).await?;
            }
            if wants_mime {
                let files: Vec<OutgoingFile<'_>> =
                    collected.iter().map(AttachmentFile::as_outgoing).collect();
                mime = Some(mime_body(
                    &self.body,
                    decision.mime_type == MimeType::Html,
                    &files,
                ));
            }
        }

        let packages = self.packed(&decision.mime_type, mime.as_deref(), &keys)?;
        let request = SendRequest {
            packages: &packages,
            auto_save_contacts: self.auto_save_contacts.map(u8::from),
            delay_seconds: self.delay_seconds,
            delivery_time: self.delivery_time,
            expires_in: self.expires_in,
            expiration_time: self.expiration_time,
        };

        let message = &self.message;
        Ok(self
            .client
            .api()
            .post(format!("{MESSAGES}/{message}"))
            .json(&request)
            .send_field("Sent")
            .await?)
    }

    fn packed(
        &self,
        mime_type: &MimeType,
        mime: Option<&str>,
        keys: &[(AttachmentId, SessionKey)],
    ) -> Result<Vec<Package>> {
        let address = self.sender.as_ref().ok_or(Error::MissingSender)?;
        let (_, key) = self.client.sending_key(address)?;

        self.recipients.packages(
            &self.client,
            &key,
            &Bodies {
                text: &self.body,
                mime_type,
                mime,
            },
            keys,
        )
    }

    async fn load_draft_attachments(&self, with_data: bool) -> Result<Vec<AttachmentFile>> {
        let draft = self.client.message(&self.message).await?;
        stream::iter(draft.attachments().to_vec())
            .map(|attachment| {
                let client = self.client.clone();
                async move {
                    let data = match with_data {
                        true => client.attachment(&attachment, &[]).await?.into_bytes(),
                        false => Vec::new(),
                    };
                    Ok::<_, Error>(AttachmentFile {
                        name: attachment.name().to_owned(),
                        mime_type: attachment.mime_type().to_owned(),
                        content_id: attachment.content_id().map(str::to_owned),
                        data,
                    })
                }
            })
            .buffered(ATTACHMENTS_AT_ONCE)
            .try_collect()
            .await
    }

    async fn attach_key(
        &self,
        address: &AddressId,
        collected: &mut Vec<AttachmentFile>,
        keys: &mut Vec<(AttachmentId, SessionKey)>,
    ) -> Result<()> {
        let (sender_address, key) = self.client.sending_key(address)?;
        let file = public_key_file(&sender_address, &key)?;
        if collected.iter().any(|known| known.name == file.name) {
            return Ok(());
        }

        let uploaded = self
            .client
            .upload(&self.message, file.name.clone(), file.armored.clone())
            .sender(address)
            .mime_type(PUBLIC_KEY_TYPE)
            .upload()
            .await?;
        keys.push((
            uploaded.id().clone(),
            self.client.attachment_session_key(&uploaded)?,
        ));
        collected.push(AttachmentFile {
            name: file.name,
            mime_type: PUBLIC_KEY_TYPE.to_owned(),
            content_id: None,
            data: file.armored.into_bytes(),
        });
        Ok(())
    }

    fn recipient(mut self, email: impl Into<String>, delivery: Delivery) -> Self {
        self.recipients.add(email, delivery);
        self
    }
}

impl fmt::Debug for SendBuilder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SendBuilder")
            .field("message", &self.message)
            .field("sender", &self.sender)
            .field("recipients", &self.recipients.len())
            .field("mime_type", &self.mime_type)
            .field("body", &self.body.len())
            .field("attachment_keys", &self.attachment_keys.len())
            .field("sign", &self.options.sign)
            .field("attach_public_key", &self.options.attach_public_key)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Serialize)]
struct SendRequest<'a> {
    #[serde(rename = "Packages")]
    packages: &'a [Package],
    #[serde(rename = "AutoSaveContacts", skip_serializing_if = "Option::is_none")]
    auto_save_contacts: Option<u8>,
    #[serde(rename = "DelaySeconds", skip_serializing_if = "Option::is_none")]
    delay_seconds: Option<u64>,
    #[serde(
        rename = "DeliveryTime",
        skip_serializing_if = "Option::is_none",
        with = "rikka_proton_api::unix_seconds::optional"
    )]
    delivery_time: Option<Timestamp>,
    #[serde(rename = "ExpiresIn", skip_serializing_if = "Option::is_none")]
    expires_in: Option<i64>,
    #[serde(
        rename = "ExpirationTime",
        skip_serializing_if = "Option::is_none",
        with = "rikka_proton_api::unix_seconds::optional"
    )]
    expiration_time: Option<Timestamp>,
}

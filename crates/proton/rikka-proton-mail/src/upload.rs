use std::fmt;

use rikka_proton_account::AddressId;
use rikka_proton_api::Multipart;

use crate::attachment::{Attachment, Disposition};
use crate::client::{ATTACHMENTS, MailClient};
use crate::error::{Error, Result};
use crate::identifier::MessageId;

const OCTET_STREAM: &str = "application/octet-stream";
const BLOB: &str = "blob";

#[must_use]
#[derive(Clone)]
pub struct AttachmentUpload {
    client: MailClient,
    message: MessageId,
    sender: Option<AddressId>,
    name: String,
    data: Vec<u8>,
    mime_type: String,
    disposition: Disposition,
    content_id: Option<String>,
}

impl AttachmentUpload {
    pub(crate) fn new(
        client: MailClient,
        message: MessageId,
        name: impl Into<String>,
        data: impl Into<Vec<u8>>,
    ) -> AttachmentUpload {
        AttachmentUpload {
            client,
            message,
            sender: None,
            name: name.into(),
            data: data.into(),
            mime_type: OCTET_STREAM.to_owned(),
            disposition: Disposition::Attachment,
            content_id: None,
        }
    }

    pub fn sender(mut self, address: &AddressId) -> Self {
        self.sender = Some(address.clone());
        self
    }

    pub fn mime_type(mut self, mime_type: impl Into<String>) -> Self {
        self.mime_type = mime_type.into();
        self
    }

    pub fn inline(mut self, content_id: impl Into<String>) -> Self {
        self.disposition = Disposition::Inline;
        self.content_id = Some(content_id.into());
        self
    }

    pub async fn upload(self) -> Result<Attachment> {
        let address = self.sender.as_ref().ok_or(Error::MissingSender)?;
        let (_, key) = self.client.sending_key(address)?;
        let pgp = self.client.pgp();

        let split = pgp.encrypt().to(key.public_key()).split(&self.data)?;
        let signature = pgp.sign().with_key(key).detached(&self.data)?;
        let (key_packets, data_packet, _) = split.into_parts();

        let mut form = Multipart::new()
            .text("MessageID", self.message.as_str())
            .text("Filename", &self.name)
            .text("MIMEType", &self.mime_type)
            .text("Disposition", self.disposition.as_str());
        if let Some(content_id) = &self.content_id {
            form = form.text("ContentID", content_id);
        }
        let form = form
            .file("KeyPackets", BLOB, key_packets)
            .file("DataPacket", BLOB, data_packet)
            .file("Signature", BLOB, signature);

        Ok(self
            .client
            .api()
            .post(ATTACHMENTS)
            .multipart(form)
            .send_field("Attachment")
            .await?)
    }
}

impl fmt::Debug for AttachmentUpload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AttachmentUpload")
            .field("message", &self.message)
            .field("name", &self.name)
            .field("mime_type", &self.mime_type)
            .field("disposition", &self.disposition)
            .field("bytes", &self.data.len())
            .finish_non_exhaustive()
    }
}

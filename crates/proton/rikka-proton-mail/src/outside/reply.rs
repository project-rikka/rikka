use std::fmt;

use rikka_proton_api::{Multipart, offload};
use rikka_proton_pgp::PublicKey;

use crate::error::{Error, Result};
use crate::outside::REPLY;
use crate::outside::message::{EncryptedOutsideReply, SentReply};
use crate::outside::session::OutsideSession;

const BLOB: &str = "blob";

#[derive(Clone)]
struct ReplyFile {
    name: String,
    mime_type: String,
    content_id: Option<String>,
    data: Vec<u8>,
}

struct EncryptedFile {
    name: String,
    mime_type: String,
    content_id: String,
    key_packets: Vec<u8>,
    data_packet: Vec<u8>,
}

#[must_use]
#[derive(Clone)]
pub struct OutsideReply {
    session: OutsideSession,
    key: Option<PublicKey>,
    body: String,
    files: Vec<ReplyFile>,
}

impl OutsideReply {
    pub(crate) fn new(session: OutsideSession, key: Option<PublicKey>) -> OutsideReply {
        OutsideReply {
            session,
            key,
            body: String::new(),
            files: Vec::new(),
        }
    }

    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = body.into();
        self
    }

    pub fn attach(
        mut self,
        name: impl Into<String>,
        mime_type: impl Into<String>,
        data: impl Into<Vec<u8>>,
    ) -> Self {
        self.files.push(ReplyFile {
            name: name.into(),
            mime_type: mime_type.into(),
            content_id: None,
            data: data.into(),
        });
        self
    }

    pub fn attach_inline(
        mut self,
        name: impl Into<String>,
        mime_type: impl Into<String>,
        content_id: impl Into<String>,
        data: impl Into<Vec<u8>>,
    ) -> Self {
        self.files.push(ReplyFile {
            name: name.into(),
            mime_type: mime_type.into(),
            content_id: Some(content_id.into()),
            data: data.into(),
        });
        self
    }

    pub async fn send(self) -> Result<Option<SentReply>> {
        let key = self.key.clone().ok_or(Error::NoReplyKey)?;
        let pgp = *self.session.pgp();
        let password = self.session.password().clone();
        let body = self.body;
        let files = self.files;

        let (encrypted_body, reply_body, encrypted_files) = offload(move || -> Result<_> {
            let encrypted_body = pgp
                .encrypt()
                .to(key.clone())
                .as_utf8()
                .armored_message(&body)?;
            let reply_body = pgp
                .encrypt()
                .with_password(password.expose())
                .as_utf8()
                .armored_message(&body)?;
            let encrypted_files = files
                .into_iter()
                .map(|file| {
                    let split = pgp.encrypt().to(key.clone()).split(&file.data)?;
                    let (key_packets, data_packet, _) = split.into_parts();
                    Ok(EncryptedFile {
                        name: file.name,
                        mime_type: file.mime_type,
                        content_id: file.content_id.unwrap_or_default(),
                        key_packets,
                        data_packet,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            Ok((encrypted_body, reply_body, encrypted_files))
        })
        .await?;

        let mut form = Multipart::new()
            .text("Body", encrypted_body)
            .text("ReplyBody", reply_body);
        for file in encrypted_files {
            form = form
                .text("Filename[]", file.name)
                .text("MIMEType[]", file.mime_type)
                .text("ContentID[]", file.content_id)
                .file("KeyPackets[]", BLOB, file.key_packets)
                .file("DataPacket[]", BLOB, file.data_packet);
        }

        let response: Option<EncryptedOutsideReply> = self
            .session
            .authorized(self.session.api().post(REPLY))
            .multipart(form)
            .send_field_or_default("Reply")
            .await?;

        let Some(reply) = response else {
            return Ok(None);
        };

        let pgp = *self.session.pgp();
        let password = self.session.password().clone();
        offload(move || reply.decrypt(&pgp, password.expose().as_bytes()).map(Some)).await
    }
}

impl fmt::Debug for OutsideReply {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OutsideReply")
            .field("session", &self.session)
            .field("body", &self.body.len())
            .field("files", &self.files.len())
            .finish_non_exhaustive()
    }
}

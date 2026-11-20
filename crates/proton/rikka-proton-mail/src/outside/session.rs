use std::fmt;

use rikka_proton_api::{Api, RequestBuilder, offload};
use rikka_proton_pgp::{Pgp, PublicKey, Secret};
use serde::Deserialize;

use crate::attachment::Attachment;
use crate::decrypted::DecryptedAttachment;
use crate::error::Result;
use crate::outside::message::{EncryptedOutsideMessage, OutsideMessage};
use crate::outside::reply::OutsideReply;
use crate::outside::{ATTACHMENT, MESSAGE, password_decryption_error};

const EO_UID_HEADER: &str = "x-eo-uid";
const AUTHORIZATION_HEADER: &str = "authorization";

#[derive(Clone)]
pub struct OutsideSession {
    api: Api,
    pgp: Pgp,
    id: String,
    token: Secret,
    password: Secret,
}

impl OutsideSession {
    pub(crate) fn new(
        api: Api,
        pgp: Pgp,
        id: String,
        token: Secret,
        password: Secret,
    ) -> OutsideSession {
        OutsideSession {
            api,
            pgp,
            id,
            token,
            password,
        }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    pub async fn message(&self) -> Result<OutsideMessage> {
        let response: MessageResponse = self.authorized(self.api.get(MESSAGE)).send().await?;

        let sender_key = response
            .public_key
            .as_deref()
            .filter(|armored| !armored.trim().is_empty())
            .map(PublicKey::import)
            .transpose()?;

        let pgp = self.pgp;
        let password = self.password.clone();
        offload(move || {
            OutsideMessage::decrypt(
                &pgp,
                password.expose().as_bytes(),
                response.message,
                sender_key,
            )
        })
        .await
    }

    pub async fn attachment(
        &self,
        message: &OutsideMessage,
        attachment: &Attachment,
    ) -> Result<DecryptedAttachment> {
        let data = self
            .authorized(self.api.get(format!("{ATTACHMENT}/{}", attachment.id())))
            .send_bytes()
            .await?;

        let pgp = self.pgp;
        let password = self.password.clone();
        let attachment = attachment.clone();
        let verifiers: Vec<PublicKey> = message.sender_key().cloned().into_iter().collect();

        offload(move || {
            let session_key = pgp
                .decrypt()
                .with_password(password.expose())
                .session_key(attachment.key_packet_bytes()?)
                .map_err(password_decryption_error)?;
            let plaintext = pgp
                .decrypt()
                .with_session_key(session_key)
                .verified_by_keys(verifiers.clone())
                .message(&data)?;
            let signature = match attachment.signature_bytes()? {
                Some(detached) => pgp
                    .verify()
                    .with_keys(verifiers)
                    .detached(plaintext.data(), detached),
                None => plaintext.signature().clone(),
            };

            Ok(DecryptedAttachment::new(&attachment, plaintext, signature))
        })
        .await
    }

    pub fn reply(&self, message: &OutsideMessage) -> OutsideReply {
        OutsideReply::new(self.clone(), message.sender_key().cloned())
    }

    pub(crate) fn pgp(&self) -> &Pgp {
        &self.pgp
    }

    pub(crate) fn password(&self) -> &Secret {
        &self.password
    }

    pub(crate) fn api(&self) -> &Api {
        &self.api
    }

    pub(crate) fn authorized(&self, request: RequestBuilder) -> RequestBuilder {
        request
            .anonymous()
            .header(AUTHORIZATION_HEADER, self.token.expose())
            .header(EO_UID_HEADER, self.id.as_str())
    }
}

impl fmt::Debug for OutsideSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OutsideSession")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
struct MessageResponse {
    #[serde(rename = "Message")]
    message: EncryptedOutsideMessage,
    #[serde(rename = "PublicKey", default)]
    public_key: Option<String>,
}

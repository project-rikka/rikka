use futures_util::future::try_join_all;
use rikka_proton_contacts::{
    MimeType as ContactMimeType, PackageScheme, PreferenceError, SendPreferences,
};
use rikka_proton_pgp::{PrivateKey, PublicKey, SessionKey};

use crate::client::MailClient;
use crate::error::{Error, Result};
use crate::identifier::AttachmentId;
use crate::mime::{MimeType, html_to_text};
use crate::package::{AddressPackage, Package, PackageKind, SignatureMode};
use crate::packaging::{self, Challenge};
use crate::send_options::Decision;

#[derive(Clone)]
pub(crate) enum Delivery {
    Resolve,
    Internal(PublicKey),
    PgpInline(PublicKey),
    PgpMime(PublicKey),
    Cleartext {
        signed: bool,
    },
    ClearMime,
    Outside {
        password: String,
        challenge: Option<Challenge>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Plain,
    Html,
    Mime,
}

impl Delivery {
    fn kind(&self) -> Option<PackageKind> {
        match self {
            Delivery::Resolve => None,
            Delivery::Internal(_) => Some(PackageKind::Internal),
            Delivery::PgpInline(_) => Some(PackageKind::PgpInline),
            Delivery::PgpMime(_) => Some(PackageKind::PgpMime),
            Delivery::Cleartext { .. } => Some(PackageKind::Cleartext),
            Delivery::ClearMime => Some(PackageKind::ClearMime),
            Delivery::Outside { .. } => Some(PackageKind::EncryptedOutside),
        }
    }

    fn signature(&self) -> SignatureMode {
        match self {
            Delivery::Cleartext { signed: false } | Delivery::PgpMime(_) | Delivery::Resolve => {
                SignatureMode::None
            }
            _ => SignatureMode::Detached,
        }
    }

    fn natural_format(&self, message: &MimeType) -> Format {
        match self {
            Delivery::PgpInline(_) => Format::Plain,
            Delivery::PgpMime(_) | Delivery::ClearMime => Format::Mime,
            _ => match message {
                MimeType::PlainText => Format::Plain,
                _ => Format::Html,
            },
        }
    }
}

pub(crate) struct Bodies<'a> {
    pub(crate) text: &'a str,
    pub(crate) mime_type: &'a MimeType,
    pub(crate) mime: Option<&'a str>,
}

#[derive(Clone)]
struct Recipient {
    email: String,
    delivery: Delivery,
    format: Option<Format>,
}

#[derive(Clone, Default)]
pub(crate) struct Recipients {
    list: Vec<Recipient>,
    password_hint: Option<String>,
}

impl Recipients {
    pub(crate) fn add(&mut self, email: impl Into<String>, delivery: Delivery) {
        let email = email.into();
        self.list
            .retain(|listed_recipient| listed_recipient.email != email);
        self.list.push(Recipient {
            email,
            delivery,
            format: None,
        });
    }

    pub(crate) fn hint(&mut self, hint: impl Into<String>) {
        self.password_hint = Some(hint.into());
    }

    pub(crate) fn len(&self) -> usize {
        self.list.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    pub(crate) fn wants_mime(&self, message: &MimeType) -> bool {
        self.list
            .iter()
            .any(|recipient| recipient.format(message) == Format::Mime)
    }

    pub(crate) async fn resolve(&mut self, client: &MailClient, decision: &Decision) -> Result<()> {
        let pending: Vec<String> = self
            .list
            .iter()
            .filter(|recipient| matches!(recipient.delivery, Delivery::Resolve))
            .map(|recipient| recipient.email.clone())
            .collect();

        let looking = pending.iter().map(|email| async move {
            client
                .contacts()
                .send_preferences(email, &decision.defaults, &decision.intent)
                .await
                .map_err(Error::from)
                .and_then(|preferences| routed(email, preferences))
        });
        let resolved = try_join_all(looking).await?;

        for (email, (delivery, format)) in pending.iter().zip(resolved) {
            if let Some(listed_recipient) = self
                .list
                .iter_mut()
                .find(|listed_recipient| listed_recipient.email == *email)
            {
                listed_recipient.delivery = delivery;
                listed_recipient.format = Some(format);
            }
        }

        self.challenge(client).await
    }

    async fn challenge(&mut self, client: &MailClient) -> Result<()> {
        let needs_challenge = self.list.iter().any(|recipient| {
            matches!(
                recipient.delivery,
                Delivery::Outside {
                    challenge: None,
                    ..
                }
            )
        });

        if !needs_challenge {
            return Ok(());
        }

        let modulus = client.account().modulus().await?;
        let pgp = *client.pgp();

        for recipient in &mut self.list {
            if let Delivery::Outside {
                password,
                challenge,
            } = &mut recipient.delivery
                && challenge.is_none()
            {
                *challenge = Some(packaging::outside_challenge(&pgp, password, &modulus)?);
            }
        }

        Ok(())
    }

    pub(crate) fn packages(
        &self,
        client: &MailClient,
        signer: &PrivateKey,
        bodies: &Bodies<'_>,
        attachment_keys: &[(AttachmentId, SessionKey)],
    ) -> Result<Vec<Package>> {
        if self.list.is_empty() {
            return Err(Error::NoRecipient);
        }

        let mut groups: Vec<(Format, Vec<&Recipient>)> = Vec::new();
        for recipient in &self.list {
            if matches!(recipient.delivery, Delivery::Resolve) {
                return Err(Error::UnresolvedRecipient(recipient.email.clone()));
            }
            let format = recipient.format(bodies.mime_type);
            match groups
                .iter_mut()
                .find(|(listed_recipient, _)| *listed_recipient == format)
            {
                Some((_, members)) => members.push(recipient),
                None => groups.push((format, vec![recipient])),
            }
        }
        groups.sort_by_key(|(format, _)| *format as u8);

        groups
            .into_iter()
            .map(|(format, members)| {
                let (body, mime_type) = match format {
                    Format::Mime => (
                        bodies.mime.ok_or(Error::MissingMimeBody)?.to_owned(),
                        MimeType::MultipartMixed,
                    ),
                    Format::Plain => (
                        match bodies.mime_type {
                            MimeType::PlainText => bodies.text.to_owned(),
                            _ => html_to_text(bodies.text),
                        },
                        MimeType::PlainText,
                    ),
                    Format::Html => (bodies.text.to_owned(), MimeType::Html),
                };
                self.package(client, signer, &body, mime_type, &members, attachment_keys)
            })
            .collect()
    }

    fn package(
        &self,
        client: &MailClient,
        signer: &PrivateKey,
        body: &str,
        mime_type: MimeType,
        recipients: &[&Recipient],
        attachment_keys: &[(AttachmentId, SessionKey)],
    ) -> Result<Package> {
        let pgp = client.pgp();
        let encrypted = packaging::encrypt_body(pgp, signer, body)?;
        let session_key = encrypted.session_key();
        let carries_attachments = !mime_type.is_mime();

        let mut package = Package::new(mime_type, encrypted.data_packet());

        for recipient in recipients {
            let email = &recipient.email;
            let delivery = &recipient.delivery;
            let kind = delivery
                .kind()
                .ok_or_else(|| Error::UnresolvedRecipient(email.clone()))?;
            let mut address = AddressPackage::new(kind, delivery.signature());

            match delivery {
                Delivery::Internal(key) | Delivery::PgpInline(key) | Delivery::PgpMime(key) => {
                    address.set_body_key_packet(packaging::key_packet_for(pgp, key, session_key)?);

                    if carries_attachments {
                        for (id, attachment_key) in attachment_keys {
                            address.add_attachment_key_packet(
                                id.clone(),
                                packaging::key_packet_for(pgp, key, attachment_key)?,
                            );
                        }
                    }
                }
                Delivery::Outside {
                    password,
                    challenge,
                } => {
                    let challenge = challenge
                        .as_ref()
                        .ok_or_else(|| Error::UnresolvedRecipient(email.clone()))?;

                    address.set_body_key_packet(packaging::key_packet_with_password(
                        pgp,
                        password,
                        session_key,
                    )?);

                    if carries_attachments {
                        for (id, attachment_key) in attachment_keys {
                            address.add_attachment_key_packet(
                                id.clone(),
                                packaging::key_packet_with_password(pgp, password, attachment_key)?,
                            );
                        }
                    }

                    address.set_outside(
                        challenge.token().to_owned(),
                        challenge.encrypted_token().to_owned(),
                        challenge.auth().clone(),
                        self.password_hint.clone(),
                    );
                }
                Delivery::Cleartext { .. } | Delivery::ClearMime => {}
                Delivery::Resolve => return Err(Error::UnresolvedRecipient(email.clone())),
            }

            package.add_address(email.clone(), address);
        }

        let kinds = package.kinds();

        if kinds.contains(PackageKind::Cleartext) || kinds.contains(PackageKind::ClearMime) {
            package.expose_body_key(session_key);
        }

        if carries_attachments && kinds.contains(PackageKind::Cleartext) {
            for (id, attachment_key) in attachment_keys {
                package.expose_attachment_key(id.clone(), attachment_key);
            }
        }

        Ok(package)
    }
}

impl Recipient {
    fn format(&self, message: &MimeType) -> Format {
        self.format
            .unwrap_or_else(|| self.delivery.natural_format(message))
    }
}

fn routed(email: &str, preferences: SendPreferences) -> Result<(Delivery, Format)> {
    if let Some(refusal) = preferences.error {
        return Err(Error::RecipientRefused(email.to_owned(), refusal));
    }

    let by_type = match preferences.mime_type {
        ContactMimeType::PlainText => Format::Plain,
        _ => Format::Html,
    };
    let key = || {
        preferences.key.clone().ok_or_else(|| {
            Error::RecipientRefused(email.to_owned(), PreferenceError::PrimaryCannotSend)
        })
    };

    Ok(match preferences.scheme {
        PackageScheme::Internal => (Delivery::Internal(key()?), by_type),
        PackageScheme::PgpMime if !preferences.sign && !preferences.encrypt => {
            (Delivery::Cleartext { signed: false }, Format::Html)
        }
        PackageScheme::PgpMime if preferences.encrypt => (Delivery::PgpMime(key()?), Format::Mime),
        PackageScheme::PgpMime => (Delivery::ClearMime, Format::Mime),
        PackageScheme::PgpInline if preferences.encrypt => {
            (Delivery::PgpInline(key()?), Format::Plain)
        }
        PackageScheme::PgpInline => (
            Delivery::Cleartext {
                signed: preferences.sign,
            },
            Format::Plain,
        ),
        _ => (Delivery::Cleartext { signed: false }, by_type),
    })
}

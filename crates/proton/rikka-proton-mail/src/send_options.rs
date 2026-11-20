use rikka_proton_account::Address;
use rikka_proton_contacts::{MailDefaults, MessageIntent, MimeType as ContactMimeType};
use rikka_proton_pgp::PrivateKey;

use crate::error::Result;
use crate::mime::MimeType;
use crate::settings::MailSettings;

pub(crate) const PUBLIC_KEY_TYPE: &str = "application/pgp-keys";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct SendOptions {
    pub(crate) sign: Option<bool>,
    pub(crate) attach_public_key: Option<bool>,
}

#[derive(Debug, Clone)]
pub(crate) struct Decision {
    pub(crate) defaults: MailDefaults,
    pub(crate) intent: MessageIntent,
    pub(crate) mime_type: MimeType,
    pub(crate) attach_public_key: bool,
}

impl SendOptions {
    pub(crate) fn decide(&self, settings: &MailSettings, mime_type: Option<&MimeType>) -> Decision {
        let mut defaults = settings.mail_defaults();
        if let Some(sign) = self.sign {
            defaults.sign = sign;
        }
        let mime_type = mime_type
            .cloned()
            .unwrap_or_else(|| settings.draft_mime_type().clone());

        Decision {
            defaults,
            intent: MessageIntent {
                encrypted_outside: false,
                sign: self.sign == Some(true),
                mime_type: Some(contact_mime(&mime_type)),
            },
            mime_type,
            attach_public_key: self
                .attach_public_key
                .unwrap_or_else(|| settings.attaches_public_key()),
        }
    }
}

pub(crate) fn contact_mime(mime_type: &MimeType) -> ContactMimeType {
    match mime_type {
        MimeType::PlainText => ContactMimeType::PlainText,
        MimeType::MultipartMixed => ContactMimeType::Multipart,
        _ => ContactMimeType::Html,
    }
}

pub(crate) struct PublicKeyFile {
    pub(crate) name: String,
    pub(crate) armored: String,
}

pub(crate) fn public_key_file(address: &Address, key: &PrivateKey) -> Result<PublicKeyFile> {
    let public = key.public_key();
    let fingerprint = public.fingerprint();
    let short: String = fingerprint.as_str().chars().take(8).collect();

    Ok(PublicKeyFile {
        name: format!(
            "publickey - {} - 0x{}.asc",
            address.email(),
            short.to_ascii_uppercase()
        ),
        armored: public.export_armored()?,
    })
}

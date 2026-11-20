use rikka_proton_pgp::PublicKey;

use crate::encryption_preferences::{EncryptionPreferences, PreferenceError};
use crate::kt::KtVerification;
use crate::scheme::{MimeType, PackageScheme, PgpScheme};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MessageIntent {
    pub encrypted_outside: bool,
    pub sign: bool,
    pub mime_type: Option<MimeType>,
}

#[derive(Debug, Clone)]
pub struct SendPreferences {
    pub encrypt: bool,
    pub sign: bool,
    pub scheme: PackageScheme,
    pub mime_type: MimeType,
    pub key: Option<PublicKey>,
    pub key_pinned: bool,
    pub has_api_keys: bool,
    pub has_pinned_keys: bool,
    pub internal: bool,
    pub encryption_disabled: bool,
    pub warnings: Vec<String>,
    pub kt: Option<KtVerification>,
    pub error: Option<PreferenceError>,
}

impl EncryptionPreferences {
    #[must_use]
    pub fn send_preferences(&self, intent: &MessageIntent) -> SendPreferences {
        let disabled = self.internal_with_disabled_e2ee;
        let encrypt = !disabled && (self.encrypt || intent.encrypted_outside);
        let sign = !(intent.encrypted_outside || disabled) && (self.sign || intent.sign);

        let scheme = if self.internal {
            PackageScheme::Internal
        } else if !self.encrypt && intent.encrypted_outside {
            PackageScheme::EncryptedOutside
        } else if sign {
            match self.scheme {
                PgpScheme::Inline => PackageScheme::PgpInline,
                PgpScheme::Mime => PackageScheme::PgpMime,
            }
        } else {
            PackageScheme::Clear
        };

        let selected = match scheme {
            PackageScheme::PgpInline if sign => Some(MimeType::PlainText),
            PackageScheme::PgpMime if sign => Some(MimeType::Multipart),
            PackageScheme::EncryptedOutside => intent.mime_type.or(self.mime_type),
            _ if intent.mime_type == Some(MimeType::PlainText) => intent.mime_type,
            _ => self.mime_type.or(intent.mime_type),
        };

        SendPreferences {
            encrypt,
            sign,
            scheme,
            mime_type: selected.unwrap_or(self.defaults.mime_type),
            key: self.send_key.clone(),
            key_pinned: self.send_key_pinned,
            has_api_keys: self.has_api_keys,
            has_pinned_keys: self.has_pinned_keys,
            internal: self.internal,
            encryption_disabled: disabled,
            warnings: self.warnings.clone(),
            kt: self.kt,
            error: self.error.clone(),
        }
    }
}

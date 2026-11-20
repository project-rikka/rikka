use std::collections::HashSet;
use std::fmt;

use rikka_proton_pgp::{Fingerprint, PublicKey};

use crate::api_keys::{ApiKeys, Recipient};
use crate::canonical::{canonical_email_by_guess, canonical_internal_email, user_id_email};
use crate::kt::{KEY_VERIFICATION_ERROR, KtVerification};
use crate::pinned_keys::PinnedKeys;
use crate::scheme::{MimeType, PgpScheme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MailDefaults {
    pub sign: bool,
    pub scheme: PgpScheme,
    pub mime_type: MimeType,
}

impl Default for MailDefaults {
    fn default() -> MailDefaults {
        MailDefaults {
            sign: false,
            scheme: PgpScheme::Mime,
            mime_type: MimeType::Html,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SelfSend {
    pub can_receive: bool,
    pub has_keys: bool,
    pub key: Option<PublicKey>,
    pub can_send: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PreferenceError {
    EmailAddress(String),
    InternalUserDisabled,
    InternalUserNoApiKey,
    PrimaryCannotSend,
    PrimaryNotPinned,
    ExternalNoValidFetchedKey,
    ExternalNoValidPinnedKey,
    ContactSignatureNotVerified,
    KeyTransparencyFailed,
}

impl fmt::Display for PreferenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PreferenceError::EmailAddress(reason) => formatter.write_str(reason),
            PreferenceError::InternalUserDisabled => formatter.write_str("Email address disabled"),
            PreferenceError::InternalUserNoApiKey => {
                formatter.write_str("No key was found for the Proton user")
            }
            PreferenceError::PrimaryCannotSend => {
                formatter.write_str("Primary key is not valid for sending")
            }
            PreferenceError::PrimaryNotPinned => {
                formatter.write_str("Trusted keys are not valid for sending")
            }
            PreferenceError::ExternalNoValidFetchedKey => {
                formatter.write_str("No external key retrieved for user is valid for sending")
            }
            PreferenceError::ExternalNoValidPinnedKey => {
                formatter.write_str("The sending key is not valid")
            }
            PreferenceError::ContactSignatureNotVerified => {
                formatter.write_str("Contact signature could not be verified")
            }
            PreferenceError::KeyTransparencyFailed => formatter.write_str(KEY_VERIFICATION_ERROR),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EncryptionPreferences {
    pub encrypt: bool,
    pub sign: bool,
    pub scheme: PgpScheme,
    pub mime_type: Option<MimeType>,
    pub internal: bool,
    pub internal_with_disabled_e2ee: bool,
    pub send_key: Option<PublicKey>,
    pub send_key_pinned: bool,
    pub api_keys: Vec<PublicKey>,
    pub pinned_keys: Vec<PublicKey>,
    pub verifying_pinned_keys: Vec<PublicKey>,
    pub has_api_keys: bool,
    pub has_pinned_keys: bool,
    pub contact: bool,
    pub contact_signature_verified: Option<bool>,
    pub contact_signature_time: Option<u64>,
    pub warnings: Vec<String>,
    pub address_warnings: Vec<String>,
    pub kt: Option<KtVerification>,
    pub error: Option<PreferenceError>,
    pub defaults: MailDefaults,
}

struct KeyModel {
    api_keys: Vec<PublicKey>,
    pinned_keys: Vec<PublicKey>,
    verifying_pinned_keys: Vec<PublicKey>,
    trusted: HashSet<Fingerprint>,
    obsolete: HashSet<Fingerprint>,
    compromised: HashSet<Fingerprint>,
    capable: HashSet<Fingerprint>,
    encrypt: Option<bool>,
}

impl KeyModel {
    fn new(api: &ApiKeys, pinned: &PinnedKeys, now: u64) -> KeyModel {
        let mut trusted = HashSet::new();
        let mut obsolete = HashSet::new();
        let mut compromised = HashSet::new();
        let mut capable = HashSet::new();
        let mut primary = HashSet::new();

        for published in api.keys() {
            let fingerprint = published.key().fingerprint();
            if published.key().can_encrypt_at(now) {
                capable.insert(fingerprint.clone());
            }
            if published.is_primary() {
                primary.insert(fingerprint.clone());
            }
            if published.is_compromised() {
                compromised.insert(fingerprint.clone());
            }
            if published.is_obsolete() {
                obsolete.insert(fingerprint);
            }
        }

        let preferences = pinned.preferences();
        for key in &preferences.pinned_keys {
            let fingerprint = key.fingerprint();
            if key.can_encrypt_at(now) {
                capable.insert(fingerprint.clone());
            }
            trusted.insert(fingerprint);
        }

        let mut pinned_keys = preferences.pinned_keys.clone();
        pinned_keys.sort_by_key(|key| {
            let fingerprint = key.fingerprint();
            sort_rank(&[
                key.version() != 4,
                obsolete.contains(&fingerprint),
                compromised.contains(&fingerprint),
                !capable.contains(&fingerprint),
            ])
        });

        let mut api_keys: Vec<PublicKey> = api
            .keys()
            .iter()
            .map(|published| published.key().clone())
            .collect();
        api_keys.sort_by_key(|key| {
            let fingerprint = key.fingerprint();
            sort_rank(&[
                key.version() != 4,
                !primary.contains(&fingerprint),
                obsolete.contains(&fingerprint),
                compromised.contains(&fingerprint),
                !trusted.contains(&fingerprint),
            ])
        });

        let verifying_pinned_keys = pinned_keys
            .iter()
            .filter(|key| !compromised.contains(&key.fingerprint()))
            .cloned()
            .collect();

        let encrypt = if !pinned_keys.is_empty() {
            Some(preferences.encrypt_to_pinned != Some(false))
        } else if !api.is_internal() && !api_keys.is_empty() {
            Some(preferences.encrypt_to_untrusted != Some(false))
        } else {
            None
        };

        KeyModel {
            api_keys,
            pinned_keys,
            verifying_pinned_keys,
            trusted,
            obsolete,
            compromised,
            capable,
            encrypt,
        }
    }

    fn can_send_to(&self, key: &PublicKey) -> bool {
        let fingerprint = key.fingerprint();
        !self.compromised.contains(&fingerprint)
            && !self.obsolete.contains(&fingerprint)
            && self.capable.contains(&fingerprint)
    }
}

fn sort_rank(bits: &[bool]) -> u32 {
    bits.iter()
        .enumerate()
        .map(|(index, bit)| u32::from(*bit) << index)
        .sum()
}

#[must_use]
pub fn encryption_preferences(
    email: &str,
    api: &ApiKeys,
    pinned: &PinnedKeys,
    defaults: &MailDefaults,
    own: Option<&SelfSend>,
    now: u64,
) -> EncryptionPreferences {
    let mut preferences = decide_preferences(api, pinned, defaults, own, now);
    if preferences.error.is_none()
        && let Some(key) = &preferences.send_key
    {
        let recipient = match own.is_some() || api.is_internal() {
            true => Recipient::Internal,
            false => Recipient::External,
        };
        preferences.warnings = email_mismatch(key, email, recipient);
    }
    preferences
}

#[must_use]
pub fn email_mismatch(key: &PublicKey, email: &str, recipient: Recipient) -> Vec<String> {
    let canonical = |address: &str| match recipient {
        Recipient::Internal => canonical_internal_email(address),
        Recipient::External => canonical_email_by_guess(address),
    };
    let target = canonical(email.trim());
    let listed: Vec<String> = key
        .user_ids()
        .iter()
        .map(|user_id| user_id_email(user_id).unwrap_or(user_id).to_owned())
        .collect();
    if listed.iter().any(|address| canonical(address) == target) {
        return Vec::new();
    }
    vec![format!(
        "Email address not found among user ids defined in sending key ({})",
        listed.join(", ")
    )]
}

fn decide_preferences(
    api: &ApiKeys,
    pinned: &PinnedKeys,
    defaults: &MailDefaults,
    own: Option<&SelfSend>,
    now: u64,
) -> EncryptionPreferences {
    let model = KeyModel::new(api, pinned, now);
    let mut preferences = base_preferences(api, pinned, defaults, &model);
    if let Some(own) = own {
        decide_self_send(&mut preferences, api, own);
    } else if let Some(reason) = api.errors().first() {
        preferences.error = Some(match api.is_kt_failure() {
            true => PreferenceError::KeyTransparencyFailed,
            false => PreferenceError::EmailAddress(reason.clone()),
        });
    } else if pinned.is_contact() && pinned.signature_verified() == Some(false) {
        preferences.error = Some(PreferenceError::ContactSignatureNotVerified);
    } else if api.is_internal() {
        decide_internal(&mut preferences, &model);
    } else if !model.api_keys.is_empty() {
        decide_external_published(&mut preferences, &model);
    } else {
        decide_external_pinned(&mut preferences, &model);
    }
    preferences
}

fn base_preferences(
    api: &ApiKeys,
    pinned: &PinnedKeys,
    defaults: &MailDefaults,
    model: &KeyModel,
) -> EncryptionPreferences {
    let contact = pinned.preferences();
    let encrypt = model.encrypt.unwrap_or(false);
    let sign = contact.sign.unwrap_or(defaults.sign);
    let scheme = contact.scheme.unwrap_or(defaults.scheme);
    let mime_type = if sign && scheme == PgpScheme::Inline {
        Some(MimeType::PlainText)
    } else {
        contact.mime_type
    };

    EncryptionPreferences {
        encrypt,
        sign: encrypt || sign,
        scheme,
        mime_type,
        internal: api.is_internal(),
        internal_with_disabled_e2ee: api.is_internal_with_disabled_e2ee(),
        send_key: None,
        send_key_pinned: false,
        api_keys: model.api_keys.clone(),
        pinned_keys: model.pinned_keys.clone(),
        verifying_pinned_keys: model.verifying_pinned_keys.clone(),
        has_api_keys: !model.api_keys.is_empty(),
        has_pinned_keys: !model.pinned_keys.is_empty(),
        contact: pinned.is_contact(),
        contact_signature_verified: pinned.signature_verified(),
        contact_signature_time: pinned.signature_time(),
        warnings: Vec::new(),
        address_warnings: api.warnings().to_vec(),
        kt: api.kt(),
        error: None,
        defaults: *defaults,
    }
}

fn decide_self_send(preferences: &mut EncryptionPreferences, api: &ApiKeys, own: &SelfSend) {
    preferences.encrypt = true;
    preferences.sign = true;
    preferences.internal = true;
    preferences.pinned_keys.clear();
    preferences.verifying_pinned_keys.clear();
    preferences.has_api_keys = own.has_keys;
    preferences.has_pinned_keys = false;
    preferences.error = if let Some(reason) = api.errors().first() {
        Some(PreferenceError::EmailAddress(reason.clone()))
    } else if !own.can_receive {
        Some(PreferenceError::InternalUserDisabled)
    } else if !own.has_keys {
        Some(PreferenceError::InternalUserNoApiKey)
    } else if own.key.is_none() || !own.can_send {
        Some(PreferenceError::PrimaryCannotSend)
    } else {
        preferences.send_key = own.key.clone();
        None
    };
}

fn decide_internal(preferences: &mut EncryptionPreferences, model: &KeyModel) {
    preferences.encrypt = true;
    preferences.sign = true;
    preferences.internal_with_disabled_e2ee = false;
    let Some(primary) = model.api_keys.first() else {
        preferences.error = Some(PreferenceError::InternalUserNoApiKey);
        return;
    };
    if !model.can_send_to(primary) {
        preferences.error = Some(PreferenceError::PrimaryCannotSend);
        return;
    }
    pick_trusted(preferences, model, primary.clone(), primary.clone());
}

fn decide_external_published(preferences: &mut EncryptionPreferences, model: &KeyModel) {
    preferences.has_api_keys = true;
    preferences.internal_with_disabled_e2ee = false;
    let primary = model.api_keys[0].clone();
    let Some(sendable) = model
        .api_keys
        .iter()
        .find(|key| model.can_send_to(key))
        .cloned()
    else {
        preferences.error = Some(PreferenceError::ExternalNoValidFetchedKey);
        return;
    };
    pick_trusted(preferences, model, primary, sendable);
}

fn decide_external_pinned(preferences: &mut EncryptionPreferences, model: &KeyModel) {
    preferences.has_api_keys = false;
    if model.pinned_keys.is_empty() || !model.encrypt.unwrap_or(false) {
        return;
    }
    let first = model.pinned_keys[0].clone();
    if !model.can_send_to(&first) {
        preferences.error = Some(PreferenceError::ExternalNoValidPinnedKey);
        return;
    }
    preferences.send_key = Some(first);
    preferences.send_key_pinned = true;
}

fn pick_trusted(
    preferences: &mut EncryptionPreferences,
    model: &KeyModel,
    primary: PublicKey,
    fallback: PublicKey,
) {
    if model.pinned_keys.is_empty() {
        preferences.send_key = Some(primary);
        preferences.send_key_pinned = false;
        return;
    }
    let fingerprint = primary.fingerprint();
    let pinned = model
        .pinned_keys
        .iter()
        .find(|key| key.fingerprint() == fingerprint);
    match pinned {
        Some(pinned) if model.trusted.contains(&fingerprint) && model.can_send_to(&primary) => {
            preferences.send_key = Some(pinned.clone());
            preferences.send_key_pinned = true;
        }
        _ => {
            preferences.send_key = Some(fallback);
            preferences.error = Some(PreferenceError::PrimaryNotPinned);
        }
    }
}

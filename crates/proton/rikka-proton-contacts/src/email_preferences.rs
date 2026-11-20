use rikka_proton_api::base64;
use rikka_proton_pgp::PublicKey;

use crate::error::{Error, Result};
use crate::property::Property;
use crate::scheme::{MimeType, PgpScheme};
use crate::vcard::VCard;

const KEY: &str = "KEY";
const ENCRYPT: &str = "X-PM-ENCRYPT";
const ENCRYPT_UNTRUSTED: &str = "X-PM-ENCRYPT-UNTRUSTED";
const SIGN: &str = "X-PM-SIGN";
const SCHEME: &str = "X-PM-SCHEME";
const MIME_TYPE: &str = "X-PM-MIMETYPE";
const KEY_PREFIX: &str = "data:application/pgp-keys;base64,";
const FIELDS: [&str; 6] = [KEY, ENCRYPT, ENCRYPT_UNTRUSTED, SIGN, SCHEME, MIME_TYPE];

#[derive(Debug, Clone, Default)]
pub struct EmailPreferences {
    pub pinned_keys: Vec<PublicKey>,
    pub encrypt_to_pinned: Option<bool>,
    pub encrypt_to_untrusted: Option<bool>,
    pub sign: Option<bool>,
    pub scheme: Option<PgpScheme>,
    pub mime_type: Option<MimeType>,
}

impl EmailPreferences {
    #[must_use]
    pub fn from_card(card: &VCard, email: &str) -> Option<EmailPreferences> {
        let group = card.email_group(email)?;
        Some(EmailPreferences::for_group(card, group))
    }

    #[must_use]
    pub fn for_group(card: &VCard, group: &str) -> EmailPreferences {
        let in_group = |name: &'static str| {
            card.properties_named(name)
                .find(move |property| property.in_group(group))
        };
        let flag = |name: &'static str| {
            in_group(name).and_then(|property| parse_bool(&property.text_value()))
        };

        let mut keys: Vec<&Property> = card
            .properties_named(KEY)
            .filter(|property| property.in_group(group))
            .collect();
        keys.sort_by_key(|property| property.pref().unwrap_or(u32::MAX));

        EmailPreferences {
            pinned_keys: keys.into_iter().filter_map(parse_pinned_key).collect(),
            encrypt_to_pinned: flag(ENCRYPT),
            encrypt_to_untrusted: flag(ENCRYPT_UNTRUSTED),
            sign: flag(SIGN),
            scheme: in_group(SCHEME)
                .and_then(|property| PgpScheme::from_name(&property.text_value())),
            mime_type: in_group(MIME_TYPE)
                .and_then(|property| MimeType::from_vcard(&property.text_value())),
        }
    }

    pub fn apply_to(&self, card: &mut VCard, email: &str) -> Result<()> {
        let group = card
            .email_group(email)
            .map(str::to_owned)
            .ok_or_else(|| Error::NoSuchEmail(email.to_owned()))?;

        card.retain(|property| {
            !(property.in_group(&group) && FIELDS.iter().any(|name| property.is(name)))
        });

        for (index, key) in self.pinned_keys.iter().enumerate() {
            let exported = key.export()?;
            card.push(
                Property::raw(KEY, format!("{KEY_PREFIX}{}", base64::encode(exported)))
                    .with_group(group.clone())
                    .with_pref(u32::try_from(index + 1).unwrap_or(u32::MAX)),
            );
        }
        let flags = [
            (ENCRYPT, self.encrypt_to_pinned),
            (ENCRYPT_UNTRUSTED, self.encrypt_to_untrusted),
            (SIGN, self.sign),
        ];
        for (name, value) in flags {
            if let Some(value) = value {
                card.push(
                    Property::text(name, if value { "true" } else { "false" })
                        .with_group(group.clone()),
                );
            }
        }
        if let Some(scheme) = self.scheme {
            card.push(Property::text(SCHEME, scheme.as_str()).with_group(group.clone()));
        }
        if self.mime_type == Some(MimeType::PlainText) {
            card.push(Property::text(MIME_TYPE, MimeType::PlainText.as_str()).with_group(group));
        }

        Ok(())
    }
}

fn parse_pinned_key(property: &Property) -> Option<PublicKey> {
    let value = property.raw_value().trim();
    let encoded = match value.split_once(',') {
        Some((_, encoded)) => encoded,
        None => value,
    };
    let compact: String = encoded
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    let bytes = base64::decode(compact).ok()?;
    if bytes.is_empty() {
        return None;
    }
    PublicKey::import(bytes).ok()
}

fn parse_bool(text_value: &str) -> Option<bool> {
    match text_value.trim().to_ascii_lowercase().as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

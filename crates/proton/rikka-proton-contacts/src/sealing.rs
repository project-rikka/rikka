use rikka_proton_pgp::{Pgp, PrivateKey};

use crate::card::{CardType, ContactCard};
use crate::error::Result;
use crate::preparation::prepare_for_saving;
use crate::property::Property;
use crate::vcard::VCard;

const CATEGORIES: &str = "CATEGORIES";
const CLEAR_FIELDS: [&str; 2] = ["PRODID", CATEGORIES];
const SIGNED_FIELDS: [&str; 11] = [
    "PRODID",
    "FN",
    "UID",
    "EMAIL",
    "KEY",
    "X-PM-MIMETYPE",
    "X-PM-ENCRYPT",
    "X-PM-ENCRYPT-UNTRUSTED",
    "X-PM-SIGN",
    "X-PM-SCHEME",
    "X-PM-TLS",
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardSplit {
    pub encrypted: VCard,
    pub signed: VCard,
    pub clear: VCard,
}

#[must_use]
pub fn split_vcard(card: &VCard) -> CardSplit {
    let has_categories = card.first(CATEGORIES).is_some();
    let mut split = CardSplit::default();

    for property in card.properties() {
        let signed = is_any_of(property, &SIGNED_FIELDS);
        if has_categories && is_any_of(property, &CLEAR_FIELDS) {
            split.clear.push(property.clone());
            if signed {
                split.signed.push(property.clone());
            }
        } else if signed {
            split.signed.push(property.clone());
        } else {
            split.encrypted.push(property.clone());
        }
    }

    split
}

fn is_any_of(property: &Property, names: &[&str]) -> bool {
    names.iter().any(|name| property.is(name))
}

pub fn seal_vcard(pgp: &Pgp, card: &VCard, key: &PrivateKey) -> Result<Vec<ContactCard>> {
    let split = split_vcard(&prepare_for_saving(card));
    let mut cards = Vec::with_capacity(3);

    if !split.encrypted.properties().is_empty() {
        let text = split.encrypted.to_card_text();
        let data = pgp
            .encrypt()
            .to(key.public_key())
            .as_utf8()
            .armored_message(&text)?;
        let signature = pgp
            .sign()
            .with_key(key.clone())
            .as_utf8()
            .armored_detached(&text)?;
        cards.push(ContactCard::new(
            CardType::EncryptedAndSigned,
            data,
            Some(signature),
        ));
    }

    let text = split.signed.to_card_text();
    let signature = pgp
        .sign()
        .with_key(key.clone())
        .as_utf8()
        .armored_detached(strip_trailing_spaces(&text))?;
    cards.push(ContactCard::new(CardType::Signed, text, Some(signature)));

    if !split.clear.properties().is_empty() {
        cards.push(ContactCard::new(
            CardType::Clear,
            split.clear.to_card_text(),
            None,
        ));
    }

    Ok(cards)
}

pub(crate) fn strip_trailing_spaces(text: &str) -> String {
    text.split('\n')
        .map(|line| line.trim_end_matches([' ', '\t', '\r']))
        .collect::<Vec<_>>()
        .join("\n")
}

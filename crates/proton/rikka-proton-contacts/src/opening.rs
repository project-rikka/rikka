use rikka_proton_pgp::{KeyRing, Pgp, SignatureStatus};

use crate::card::{CardType, ContactCard};
use crate::decrypted_contact::{CardProblem, DecryptedContact};
use crate::merging::combine_card_parts;
use crate::sealing::strip_trailing_spaces;
use crate::vcard::VCard;

const UID: &str = "UID";

#[must_use]
pub fn decrypt_cards(pgp: &Pgp, cards: &[ContactCard], keys: &KeyRing) -> DecryptedContact {
    let mut verified = cards.iter().any(|card| card.kind().is_signed());
    let mut parsed = Vec::with_capacity(cards.len());
    let mut problems = Vec::new();

    for card in cards {
        match read_card(pgp, card, keys) {
            Ok((text, status)) => {
                if let Some(status) = status.filter(|status| !status.is_valid()) {
                    verified = false;
                    problems.push(CardProblem::NotVerified {
                        card: card.kind(),
                        status,
                    });
                }
                match VCard::parse(&text) {
                    Ok(mut vcard) => {
                        if card.kind().is_encrypted() {
                            vcard.remove_named(UID);
                        }
                        parsed.push(vcard);
                    }
                    Err(error) => problems.push(CardProblem::Unreadable {
                        card: card.kind(),
                        reason: error.to_string(),
                    }),
                }
            }
            Err(problem) => {
                if card.kind().is_signed() {
                    verified = false;
                }
                problems.push(problem);
            }
        }
    }

    DecryptedContact::new(combine_card_parts(parsed), verified, problems)
}

fn read_card(
    pgp: &Pgp,
    card: &ContactCard,
    keys: &KeyRing,
) -> Result<(String, Option<SignatureStatus>), CardProblem> {
    match card.kind() {
        CardType::Clear => Ok((card.data().to_owned(), None)),
        CardType::Signed => {
            let status = match card.signature() {
                Some(signature) => pgp
                    .verify()
                    .with_ring(keys)
                    .as_utf8()
                    .detached(strip_trailing_spaces(card.data()), signature),
                None => SignatureStatus::Missing,
            };
            Ok((card.data().to_owned(), Some(status)))
        }
        CardType::Encrypted => Ok((decrypted(pgp, card, keys)?, None)),
        CardType::EncryptedAndSigned => {
            let text = decrypted(pgp, card, keys)?;
            let status = match card.signature() {
                Some(signature) => pgp
                    .verify()
                    .with_ring(keys)
                    .as_utf8()
                    .detached(&text, signature),
                None => SignatureStatus::Missing,
            };
            Ok((text, Some(status)))
        }
        CardType::Other(kind) => Err(CardProblem::UnknownType { kind }),
    }
}

fn decrypted(pgp: &Pgp, card: &ContactCard, keys: &KeyRing) -> Result<String, CardProblem> {
    let plaintext = pgp
        .decrypt()
        .with_ring(keys)
        .as_utf8()
        .message(card.data())
        .map_err(|error| CardProblem::NotDecrypted {
            card: card.kind(),
            reason: error.to_string(),
        })?;
    String::from_utf8(plaintext.into_data()).map_err(|error| CardProblem::Unreadable {
        card: card.kind(),
        reason: error.to_string(),
    })
}

use std::fmt;

use rikka_proton_pgp::SignatureStatus;

use crate::card::CardType;
use crate::vcard::VCard;

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CardProblem {
    NotVerified {
        card: CardType,
        status: SignatureStatus,
    },
    NotDecrypted {
        card: CardType,
        reason: String,
    },
    Unreadable {
        card: CardType,
        reason: String,
    },
    UnknownType {
        kind: i32,
    },
}

impl fmt::Display for CardProblem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CardProblem::NotVerified { card, status } => {
                write!(formatter, "{card:?} card signature is not valid: {status}")
            }
            CardProblem::NotDecrypted { card, reason } => {
                write!(formatter, "failed to decrypt {card:?} card: {reason}")
            }
            CardProblem::Unreadable { card, reason } => {
                write!(formatter, "{card:?} card is not a valid vCard: {reason}")
            }
            CardProblem::UnknownType { kind } => {
                write!(formatter, "unknown card type {kind}")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecryptedContact {
    vcard: VCard,
    verified: bool,
    problems: Vec<CardProblem>,
}

impl DecryptedContact {
    pub(crate) fn new(
        vcard: VCard,
        verified: bool,
        problems: Vec<CardProblem>,
    ) -> DecryptedContact {
        DecryptedContact {
            vcard,
            verified,
            problems,
        }
    }

    #[must_use]
    pub fn vcard(&self) -> &VCard {
        &self.vcard
    }

    #[must_use]
    pub fn into_vcard(self) -> VCard {
        self.vcard
    }

    #[must_use]
    pub fn is_verified(&self) -> bool {
        self.verified
    }

    #[must_use]
    pub fn problems(&self) -> &[CardProblem] {
        &self.problems
    }

    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.problems
            .iter()
            .all(|problem| matches!(problem, CardProblem::NotVerified { .. }))
    }
}

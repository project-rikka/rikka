use crate::identifier::ContactId;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Api(#[from] rikka_proton_api::Error),

    #[error(transparent)]
    Account(#[from] rikka_proton_account::Error),

    #[error(transparent)]
    Pgp(#[from] rikka_proton_pgp::Error),

    #[error(transparent)]
    ContentLine(#[from] rikka_proton_contentline::Error),

    #[error("no vCard found")]
    NoVCard,

    #[error("expected one vCard, found {0}")]
    SeveralVCards(usize),

    #[error("unbalanced BEGIN/END in vCard")]
    UnbalancedVCard,

    #[error("vCard has no email {0}")]
    NoSuchEmail(String),

    #[error("account has no unlocked user key")]
    NoUserKey,

    #[error("merge requires at least two contacts")]
    NothingToMerge,

    #[error("contact {0} has no cards")]
    NoCards(ContactId),

    #[error("contact {0} has unreadable cards: {1}")]
    UnreadableContact(ContactId, String),

    #[error("API response has no saved contact")]
    MissingContact,

    #[error("contact group name is empty")]
    MissingGroupName,
}

impl Error {
    #[must_use]
    pub fn api(&self) -> Option<&rikka_proton_api::Error> {
        match self {
            Error::Api(error) => Some(error),
            Error::Account(error) => error.api(),
            _ => None,
        }
    }
}

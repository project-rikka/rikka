use crate::identifier::{AttachmentId, LabelId};

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

    #[error("message has no sender address")]
    MissingSender,

    #[error("no unlocked address key")]
    NoUnlockedKey,

    #[error("message has no recipients")]
    NoRecipient,

    #[error("no label named {0}")]
    NoSuchLabel(LabelId),

    #[error("recipient {0} is unresolved")]
    UnresolvedRecipient(String),

    #[error("cannot send to {0}: {1}")]
    RecipientRefused(String, rikka_proton_contacts::PreferenceError),

    #[error(transparent)]
    Contacts(#[from] rikka_proton_contacts::Error),

    #[error("PGP/MIME package requires a multipart/mixed body")]
    MissingMimeBody,

    #[error("attachment {0} has no key packets")]
    NoKeyPackets(AttachmentId),

    #[error("system label {0} cannot be modified")]
    SystemLabel(LabelId),

    #[error("label name is missing")]
    MissingLabelName,

    #[error("{0} is not valid base64")]
    NotBase64(&'static str),

    #[error("incorrect message password")]
    WrongPassword,

    #[error("invalid encrypted-outside link {0:?}")]
    InvalidOutsideLink(String),

    #[error("sender has no public key for reply")]
    NoReplyKey,

    #[error("imported message {0} exceeds import size limit")]
    ImportTooLarge(usize),

    #[error("failed to decrypt search index")]
    IndexDecryption,

    #[error("filter name is missing")]
    MissingFilterName,

    #[error("filter requires a simple rule or sieve script")]
    MissingFilterRule,

    #[error("failed to encode {what}: {source}")]
    Encoding {
        what: &'static str,
        source: serde_json::Error,
    },
}

impl Error {
    #[must_use]
    pub fn api(&self) -> Option<&rikka_proton_api::Error> {
        match self {
            Error::Api(error) => Some(error),
            Error::Account(error) => error.api(),
            Error::Contacts(error) => error.api(),
            _ => None,
        }
    }
}

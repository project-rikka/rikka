use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("mailbox: {0}")]
    Mail(#[from] rikka_proton_mail::Error),

    #[error("no sender address from this account")]
    NoSender,

    #[error("no recipients")]
    NoRecipient,

    #[error(transparent)]
    Shared(Arc<Error>),

    #[error("message fetch was cancelled")]
    Abandoned,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

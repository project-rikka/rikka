use std::error::Error as StdError;

use crate::error::Error;

type Source = Box<dyn StdError + Send + Sync>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FeedError {
    #[error("signed out: {0}")]
    SignedOut(Source),

    #[error(transparent)]
    Failed(Source),

    #[error("invalid event payload: {0}")]
    InvalidEvent(#[from] serde_json::Error),
}

impl FeedError {
    pub fn new<E>(error: E, api: impl FnOnce(&E) -> Option<&rikka_proton_api::Error>) -> FeedError
    where
        E: StdError + Send + Sync + 'static,
    {
        match api(&error).is_some_and(rikka_proton_api::Error::is_signed_out) {
            true => FeedError::SignedOut(Box::new(error)),
            false => FeedError::Failed(Box::new(error)),
        }
    }

    #[must_use]
    pub fn is_signed_out(&self) -> bool {
        matches!(self, FeedError::SignedOut(_))
    }
}

impl From<Error> for FeedError {
    fn from(error: Error) -> FeedError {
        FeedError::new(error, Error::api)
    }
}

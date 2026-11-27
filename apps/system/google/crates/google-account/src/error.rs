#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the Google sign-in is no longer valid")]
    SignedOut,

    #[error("Google asks to finish signing in on the web")]
    NeedsBrowser,

    #[error("Google refused the request: {0}")]
    Refused(String),

    #[error("failed to reach Google: {0}")]
    Unreachable(String),

    #[error("malformed answer from Google: {0}")]
    Malformed(&'static str),

    #[error("invalid sign-in endpoint: {0}")]
    Endpoint(String),

    #[error(transparent)]
    Net(#[from] rikka_net::Error),

    #[error(transparent)]
    Storage(#[from] rikka_account_book::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    #[must_use]
    pub fn is_signed_out(&self) -> bool {
        matches!(self, Error::SignedOut)
    }

    pub(crate) fn refusal(reason: &str) -> Error {
        match reason {
            "BadAuthentication"
            | "InvalidSecondFactor"
            | "NotVerified"
            | "AccountDeleted"
            | "AccountDisabled"
            | "INVALID_GRANT" => Error::SignedOut,
            "NeedsBrowser" | "CaptchaRequired" => Error::NeedsBrowser,
            other => Error::Refused(other.to_owned()),
        }
    }
}

use browser_translators::Error;
use rikka_app::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TranslateFailure {
    Offline,
    Busy,
    Unsupported,
    Service,
}

impl TranslateFailure {
    pub(crate) fn from_error(error: &Error) -> TranslateFailure {
        match error {
            Error::Network(_) | Error::Timeout => TranslateFailure::Offline,
            Error::RateLimited => TranslateFailure::Busy,
            Error::Unsupported { .. } => TranslateFailure::Unsupported,
            _ => TranslateFailure::Service,
        }
    }

    pub(crate) fn label(self) -> Label {
        match self {
            TranslateFailure::Offline => tr!("browser.translate.failure-offline"),
            TranslateFailure::Busy => tr!("browser.translate.failure-busy"),
            TranslateFailure::Unsupported => tr!("browser.translate.failure-unsupported"),
            TranslateFailure::Service => tr!("browser.translate.failure-service"),
        }
    }
}

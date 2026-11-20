#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("udisks2 is not available")]
    Unavailable,
    #[error("not authorized")]
    NotAuthorized,
    #[error("device is busy")]
    Busy,
    #[error("operation not supported by device")]
    NotSupported,
    #[error("already mounted")]
    AlreadyMounted,
    #[error("not mounted")]
    NotMounted,
    #[error("wrong passphrase")]
    WrongPassphrase,
    #[error("udisks2 refused: {message}")]
    Refused { message: String },
}

impl From<zbus::Error> for Error {
    fn from(error: zbus::Error) -> Self {
        match &error {
            zbus::Error::MethodError(name, message, _) => {
                refusal(name.as_str(), message.as_deref().unwrap_or_default())
            }
            other => Self::Refused {
                message: other.to_string(),
            },
        }
    }
}

fn refusal(name: &str, message: &str) -> Error {
    match name.rsplit_once('.').map(|(_, last)| last) {
        Some("NotAuthorized" | "NotAuthorizedCanObtain" | "NotAuthorizedDismissed") => {
            Error::NotAuthorized
        }
        Some("DeviceBusy" | "AlreadyUnmounting") => Error::Busy,
        Some("NotSupported" | "OptionNotPermitted") => Error::NotSupported,
        Some("AlreadyMounted") => Error::AlreadyMounted,
        Some("NotMounted") => Error::NotMounted,
        _ if message.to_lowercase().contains("passphrase") => Error::WrongPassphrase,
        _ => Error::Refused {
            message: message.to_owned(),
        },
    }
}

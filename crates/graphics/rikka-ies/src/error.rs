/// Why an LM-63 file could not be read.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum IesError {
    #[error("missing TILT line")]
    MissingTilt,
    #[error("unsupported tilt {0:?}; only TILT=NONE is read")]
    Tilt(String),
    #[error("profile ends before its {0}")]
    Truncated(&'static str),
    #[error("invalid {what}: {token:?}")]
    Number { what: &'static str, token: String },
    #[error("unsupported photometric type {0}; only type C is read")]
    PhotometricType(u32),
    #[error("invalid {what} angles: {reason}")]
    Angles {
        what: &'static str,
        reason: &'static str,
    },
}

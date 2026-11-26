pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to read or write the file: {0}")]
    Io(#[from] std::io::Error),
    #[error("not an age file")]
    NotAge,
    #[error("ASCII-armored age files are not supported")]
    Armored,
    #[error("malformed age header: {0}")]
    Header(&'static str),
    #[error("the file is encrypted to recipients, not to a passphrase")]
    NotPassphrase,
    #[error("work factor {factor} exceeds the allowed {allowed}")]
    WorkFactor { factor: u8, allowed: u8 },
    #[error("incorrect passphrase")]
    Passphrase,
    #[error("the header failed authentication")]
    Tampered,
    #[error("the payload failed authentication")]
    Payload,
    #[error("the payload is truncated")]
    Truncated,
    #[error("failed to draw random bytes: {0}")]
    Random(getrandom::Error),
}

impl From<Error> for std::io::Error {
    fn from(error: Error) -> Self {
        match error {
            Error::Io(error) => error,
            error => std::io::Error::new(std::io::ErrorKind::InvalidData, error),
        }
    }
}

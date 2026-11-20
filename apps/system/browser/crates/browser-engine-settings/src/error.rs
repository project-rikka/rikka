#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum InputError {
    #[error("address is not a valid URL")]
    MalformedUrl,
    #[error("address must use HTTPS")]
    NotHttps,
    #[error("address must use HTTP, HTTPS or a local file")]
    UnsupportedScheme,
    #[error("endpoint has no host")]
    MissingHost,
    #[error("endpoint needs a port between 1 and 65535")]
    InvalidPort,
}

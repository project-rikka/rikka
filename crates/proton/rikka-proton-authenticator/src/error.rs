use rikka_proton_pgp::SignatureStatus;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not an otpauth URI")]
    MalformedUri,

    #[error("unsupported OTP URI scheme \"{scheme}\"")]
    UnsupportedScheme { scheme: String },

    #[error("unsupported OTP type \"{kind}\"")]
    UnsupportedType { kind: String },

    #[error("OTP URI has no secret")]
    MissingSecret,

    #[error("OTP secret is empty")]
    EmptySecret,

    #[error("OTP secret is not valid base32")]
    MalformedSecret,

    #[error("invalid base32")]
    MalformedBase32,

    #[error("unsupported algorithm \"{name}\"")]
    UnsupportedAlgorithm { name: String },

    #[error("invalid digit count \"{digits}\"")]
    InvalidDigits { digits: String },

    #[error("invalid period \"{period}\"")]
    InvalidPeriod { period: String },

    #[error("invalid counter \"{counter}\"")]
    InvalidCounter { counter: String },

    #[error("invalid {subject} protobuf: {reason}")]
    Malformed {
        subject: &'static str,
        reason: String,
    },

    #[error("entry has no content")]
    EmptyEntry,

    #[error("migration payload is not valid base64")]
    MalformedPayload,

    #[error(transparent)]
    Api(#[from] rikka_proton_api::Error),

    #[error(transparent)]
    Pgp(#[from] rikka_proton_pgp::Error),

    #[error(transparent)]
    Account(#[from] rikka_proton_account::Error),

    #[error("authenticator key must be 32 bytes")]
    InvalidKeyLength,

    #[error("authenticator key is not valid base64")]
    MalformedKey,

    #[error("invalid authenticator key signature: {status}")]
    InvalidKeySignature { status: Box<SignatureStatus> },

    #[error("no authenticator key decrypts the entries")]
    NoUsableKey,

    #[error("malformed entry content")]
    MalformedContent,

    #[error("failed to decrypt entry content")]
    UndecryptableContent,

    #[error("API response has no entry")]
    MissingEntry,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub(crate) fn malformed(subject: &'static str, error: prost::DecodeError) -> Error {
        Error::Malformed {
            subject,
            reason: error.to_string(),
        }
    }
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

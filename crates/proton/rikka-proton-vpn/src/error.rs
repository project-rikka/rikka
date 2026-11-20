use crate::config::Protocol;
use crate::servers::Tier;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Api(#[from] rikka_proton_api::Error),

    #[error(transparent)]
    Account(Box<rikka_proton_account::Error>),

    #[error(transparent)]
    Network(#[from] rikka_network::Error),

    #[error("{subject} is not valid base64")]
    NotBase64 { subject: &'static str },

    #[error("{subject} is {length} bytes, expected 32")]
    KeyLength {
        subject: &'static str,
        length: usize,
    },

    #[error("unknown protocol \"{name}\"")]
    UnknownProtocol { name: String },

    #[error("server {server} has no {protocol} entry")]
    MissingEntry { server: String, protocol: Protocol },

    #[error("server {server} has no X25519 public key")]
    MissingServerKey { server: String },

    #[error("server {server} has no port for {protocol}")]
    NoPort { server: String, protocol: Protocol },

    #[error("malformed binary server status: {reason}")]
    MalformedStatus { reason: &'static str },

    #[error("invalid local agent client certificate: {reason}")]
    AgentCertificate { reason: String },

    #[error("no usable local agent root certificate")]
    AgentRoots,

    #[error("failed to encrypt OpenVPN private key: {reason}")]
    KeyEncryption { reason: String },

    #[error("invalid TLS server name \"{name}\"")]
    InvalidServerName { name: String },

    #[error("no matching server")]
    NoServer,

    #[error("server {server} requires tier {tier}")]
    InsufficientTier { server: String, tier: Tier },

    #[error("tunnel backend does not support {protocol}")]
    UnsupportedProtocol { protocol: Protocol },

    #[error("sign-in code expired")]
    SignInExpired,
}

impl From<rikka_proton_account::Error> for Error {
    fn from(error: rikka_proton_account::Error) -> Error {
        match error {
            rikka_proton_account::Error::Api(error) => Error::Api(error),
            other => Error::Account(Box::new(other)),
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

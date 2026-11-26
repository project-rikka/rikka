#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Network(#[from] rikka_net::Error),
    #[error("Microsoft sign-in failed: {0}")]
    Microsoft(#[from] rikka_oauth::Error),
    #[error("the keyring is unavailable: {0}")]
    Keyring(#[from] rikka_secrets::Error),
    #[error("invalid {step} response: {source}")]
    Response {
        step: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("{step} failed with status {status}")]
    Rejected { step: &'static str, status: u16 },
    #[error("this Microsoft account has no Xbox profile")]
    NoXboxAccount,
    #[error("Xbox Live is not available in this account's country")]
    XboxUnavailable,
    #[error("this account belongs to a child and must be added to a Microsoft family")]
    ChildAccount,
    #[error("Xbox Live refused the account (error {0})")]
    Xbox(u64),
    #[error("Minecraft services are rate limiting sign-ins; try again later")]
    RateLimited,
    #[error("this account does not own Minecraft: Java Edition")]
    NoLicense,
    #[error("this account has no Minecraft profile yet; create one at minecraft.net")]
    NoProfile,
    #[error("the sign-in expired; sign in again")]
    Expired,
    #[error("the account {0} has no stored sign-in")]
    NotSignedIn(String),
    #[error("failed to save accounts: {0}")]
    Save(#[source] std::io::Error),
    #[error("failed to read accounts: {0}")]
    Load(#[source] std::io::Error),
    #[error("the system has no randomness: {0}")]
    Random(getrandom::Error),
    #[error("the stored device key is damaged")]
    DeviceKey,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

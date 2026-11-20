#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("pass: {0}")]
    Pass(#[from] rikka_proton_pass::Error),

    #[error("an alias needs a domain")]
    NoSuffix,

    #[error("this item needs a name")]
    NoTitle,

    #[error("vault not found")]
    NoVault,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("invalid BCP 47 language tag `{0}`")]
    InvalidLanguage(String),
    #[error("invalid feature setting `{0}`")]
    InvalidFeature(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

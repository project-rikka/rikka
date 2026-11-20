#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProfileIssue {
    #[error("embedded ICC profile is unreadable: {0}")]
    Unreadable(String),
    #[error("embedded ICC profile does not describe CMYK")]
    NotCmyk,
    #[error("cannot build a transform from the embedded ICC profile: {0}")]
    Transform(String),
}

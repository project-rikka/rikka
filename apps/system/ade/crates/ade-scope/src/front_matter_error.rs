#[derive(Debug, thiserror::Error)]
pub enum FrontMatterError {
    #[error("missing front matter: the file must start with ---")]
    Missing,

    #[error("front matter is not closed with ---")]
    Unclosed,

    #[error("front matter is not what was expected: {0}")]
    Malformed(#[source] serde_json::Error),
}

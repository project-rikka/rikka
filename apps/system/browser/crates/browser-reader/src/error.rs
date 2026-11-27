use std::fmt;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("page is not readable")]
    NotReadable,
    #[error("malformed reader result: {0} is missing or has the wrong type")]
    Malformed(&'static str),
    #[error("article exceeds the {0} limit")]
    TooLarge(Limit),
    #[error("failed to run the reader script: {0}")]
    Script(#[from] rikka_web::Error),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Limit {
    Blocks,
    Text,
    Nesting,
    Images,
    Inlines,
    Rows,
    Cells,
}

impl fmt::Display for Limit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Limit::Blocks => "blocks",
            Limit::Text => "text",
            Limit::Nesting => "nesting",
            Limit::Images => "images",
            Limit::Inlines => "inline runs",
            Limit::Rows => "table rows",
            Limit::Cells => "table cells",
        })
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

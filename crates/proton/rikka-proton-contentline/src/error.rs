pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("line {line}: missing property name")]
    MissingName { line: usize },

    #[error("line {line}: missing value separator")]
    MissingValue { line: usize },

    #[error("line {line}: parameter without a name")]
    MissingParameterName { line: usize },

    #[error("line {line}: unterminated quoted parameter value")]
    UnclosedQuote { line: usize },

    #[error("line {line}: invalid character {found:?} in property name")]
    InvalidName { line: usize, found: char },
}

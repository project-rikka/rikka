#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("\"{0}\" was made optional without being declared")]
    Undeclared(String),

    #[error("\"{0}\" declares parameters on top of a schema written out in full")]
    Overdeclared(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

use crate::passwd::PASSWD;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("`{0}` contains a NUL byte")]
    Nul(String),
    #[error("failed to spawn authentication thread: {0}")]
    Spawn(#[source] std::io::Error),
    #[error(
        "failed to start PAM helper {path}; enable `rikka.enable` in the system configuration: {source}"
    )]
    Helper {
        path: &'static str,
        #[source]
        source: std::io::Error,
    },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum PasswordError {
    #[error("the current password is incorrect")]
    Incorrect,
    #[error("the new password was refused")]
    Rejected { reason: Option<String> },
    #[error("passwords cannot be changed here: {PASSWD} is missing")]
    Unavailable,
    #[error("passwd asked an unexpected question: {0}")]
    Unexpected(String),
    #[error("passwd failed: {0}")]
    Failed(String),
    #[error("failed to run passwd: {0}")]
    Io(#[from] std::io::Error),
}

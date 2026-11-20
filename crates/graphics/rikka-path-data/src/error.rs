#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    NoMoveFirst,
    UnknownCommand(char),
    Number,
    Flag,
    UnexpectedEnd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{kind} at byte {offset}")]
pub struct Error {
    pub offset: usize,
    pub kind: ErrorKind,
}

impl std::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorKind::NoMoveFirst => f.write_str("path data does not start with M or m"),
            ErrorKind::UnknownCommand(command) => write!(f, "unknown path command {command:?}"),
            ErrorKind::Number => f.write_str("expected a number"),
            ErrorKind::Flag => f.write_str("expected an arc flag 0 or 1"),
            ErrorKind::UnexpectedEnd => f.write_str("path data ends inside a command"),
        }
    }
}

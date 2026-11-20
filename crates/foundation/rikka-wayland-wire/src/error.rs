use std::io;

use crate::ids::MAX_FDS_PER_MESSAGE;

#[derive(Debug, thiserror::Error)]
pub enum WireError {
    #[error("message size {0} is not a multiple of four between 8 and 4096 bytes")]
    BadSize(usize),
    #[error("message truncated before its last argument")]
    Truncated,
    #[error("{0} trailing bytes after the last argument")]
    TrailingBytes(usize),
    #[error("string is not NUL-terminated")]
    Unterminated,
    #[error("string is not UTF-8")]
    NotUtf8,
    #[error("non-nullable {0} argument is null")]
    Null(&'static str),
    #[error("argument {index} read as {read}, signature has {expected}")]
    Signature {
        index: usize,
        read: &'static str,
        expected: &'static str,
    },
    #[error("expected a file descriptor, none received")]
    MissingFd,
    #[error("more than {MAX_FDS_PER_MESSAGE} file descriptors in one message")]
    TooManyFds,
    #[error("object {0} does not exist")]
    UnknownObject(u32),
    #[error("object {id} is a {found}, not a {expected}")]
    WrongInterface {
        id: u32,
        found: &'static str,
        expected: &'static str,
    },
    #[error("{0} is not a valid new object id")]
    InvalidNewId(u32),
    #[error("opcode {0} does not exist")]
    UnknownOpcode(u16),
    #[error("event refers to a destroyed object")]
    DeadObject,
    #[error("event refers to an object of another client")]
    ForeignObject,
    #[error("outgoing buffer exceeds 4 MiB")]
    Backlog,
    #[error(transparent)]
    Io(#[from] io::Error),
}

pub type Result<T, E = WireError> = std::result::Result<T, E>;

use std::fmt;
use std::io;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Disconnect {
    Hangup,
    Protocol {
        interface: &'static str,
        object: u32,
        code: u32,
        message: String,
    },
    Backlog,
    Io(io::ErrorKind),
    Requested,
}

impl fmt::Display for Disconnect {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hangup => formatter.write_str("client hung up"),
            Self::Protocol {
                interface,
                object,
                code,
                message,
            } => write!(
                formatter,
                "protocol error {code} on {interface}#{object}: {message}"
            ),
            Self::Backlog => formatter.write_str("client stopped reading events"),
            Self::Io(kind) => write!(formatter, "connection failed: {kind}"),
            Self::Requested => formatter.write_str("disconnected by the compositor"),
        }
    }
}

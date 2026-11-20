use std::io;

use x11rb_protocol::errors::ParseError;
use x11rb_protocol::x11_utils::X11Error;

use crate::error::Error;

#[derive(Debug)]
pub(crate) enum Fault {
    Lost(io::Error),
    Refused(X11Error),
    Exhausted,
    Oversized(usize),
    Missing(&'static str),
}

pub(crate) type Outcome<T = ()> = std::result::Result<T, Fault>;

impl Fault {
    pub(crate) fn closed() -> Self {
        Self::Lost(io::Error::from(io::ErrorKind::UnexpectedEof))
    }
}

impl From<io::Error> for Fault {
    fn from(error: io::Error) -> Self {
        Self::Lost(error)
    }
}

impl From<rustix::io::Errno> for Fault {
    fn from(error: rustix::io::Errno) -> Self {
        Self::Lost(error.into())
    }
}

impl From<ParseError> for Fault {
    fn from(error: ParseError) -> Self {
        Self::Lost(io::Error::new(io::ErrorKind::InvalidData, error))
    }
}

impl From<X11Error> for Fault {
    fn from(error: X11Error) -> Self {
        Self::Refused(error)
    }
}

impl From<Fault> for Error {
    fn from(fault: Fault) -> Self {
        match fault {
            Fault::Lost(error) => {
                tracing::debug!("X11 window manager connection failed: {error}");
                Self::Disconnected
            }
            Fault::Refused(error) => Self::Refused(describe(&error)),
            Fault::Exhausted => Self::Refused("X resource identifiers ran out".into()),
            Fault::Oversized(length) => Self::Refused(format!(
                "{length}-byte request exceeds the Xwayland maximum request length"
            )),
            Fault::Missing(extension) => Self::MissingExtension(extension),
        }
    }
}

pub(crate) fn describe(error: &X11Error) -> String {
    format!(
        "{:?} from {}",
        error.error_kind,
        error.request_name.unwrap_or("an unnamed request")
    )
}

use crate::part::Part;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unknown body part `{0}`")]
    UnknownPart(String),
    #[error("unknown key tag `{0}`")]
    UnknownTag(String),
    #[error(transparent)]
    Hold(#[from] animate_hold::Error),
    #[error("unknown gaze mode `{0}`")]
    UnknownGaze(String),
    #[error("{} key at frame {frame} holds values of another part", part.name())]
    InvalidKey { part: Part, frame: u32 },
    #[error("duplicate {} key at frame {frame}", part.name())]
    DuplicateKey { part: Part, frame: u32 },
    #[error("{track} key at frame {frame} is not finite or not a unit rotation")]
    Malformed { track: String, frame: u32 },
    #[error("{0} is out of range")]
    OutOfRange(&'static str),
    #[error("duplicate {track} key at frame {frame}")]
    Repeated { track: String, frame: u32 },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

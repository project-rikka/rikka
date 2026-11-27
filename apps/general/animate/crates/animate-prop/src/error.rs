#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unknown prop setting `{0}`")]
    UnknownSetting(String),
    #[error("prop {0} is out of range")]
    OutOfRange(String),
    #[error("invalid {setting} key value at frame {frame}")]
    InvalidKey { setting: String, frame: u32 },
    #[error("duplicate {setting} key at frame {frame}")]
    DuplicateKey { setting: String, frame: u32 },
    #[error("invalid attachment at frame {0}")]
    InvalidAnchor(u32),
    #[error("duplicate attachment key at frame {0}")]
    DuplicateAnchor(u32),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

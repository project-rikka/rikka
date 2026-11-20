#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unknown emitter setting `{0}`")]
    UnknownSetting(String),
    #[error("unknown liquid property `{0}`")]
    UnknownProperty(String),
    #[error("unknown liquid preset `{0}`")]
    UnknownPreset(String),
    #[error("unknown aim `{0}`")]
    UnknownAim(String),
    #[error("invalid {setting} key value at frame {frame}")]
    InvalidKey { setting: String, frame: u32 },
    #[error("duplicate {setting} key at frame {frame}")]
    DuplicateKey { setting: String, frame: u32 },
    #[error("{name} is out of range")]
    OutOfRange { name: String },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

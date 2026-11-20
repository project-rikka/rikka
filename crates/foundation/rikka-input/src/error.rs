#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("empty chord")]
    EmptyChord,
    #[error("unknown modifier `{0}`; expected shift, ctrl, alt or super")]
    UnknownModifier(String),
    #[error("unknown key `{0}`")]
    UnknownKey(String),
    #[error("modifier `{0}` is named twice")]
    RepeatedModifier(String),
    #[error("failed to read keyboard layouts: {0}")]
    LayoutList(String),
    #[error("failed to read keymap: {0}")]
    KeymapRead(String),
    #[error("keymap failed to compile")]
    InvalidKeymap,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

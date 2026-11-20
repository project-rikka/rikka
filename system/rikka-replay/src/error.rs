use std::path::PathBuf;

use rikka_video::VideoCodec;

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("no session socket: {0}")]
    NoSession(std::io::Error),

    #[error("cannot open settings: {0}")]
    Settings(#[from] rikka_settings::Error),

    #[error("seam error: {0}")]
    Seam(#[from] rikka_seam::SeamError),

    #[error("cannot start worker: {0}")]
    Workers(std::io::Error),

    #[error("no replay buffer for `{0}`")]
    UnknownSubject(String),

    #[error("replay buffer is empty")]
    Empty,

    #[error("instant replay is disabled")]
    Disabled,

    #[error("no folder to save replays in")]
    NoFolder,

    #[error("cannot create {folder}: {error}", folder = .0.display(), error = .1)]
    CreateFolder(PathBuf, std::io::Error),

    #[error("too many replays saved this second in {folder}", folder = .0.display())]
    NameCollision(PathBuf),

    #[error("cannot write {file}: {error}", file = .0.display(), error = .1)]
    Write(PathBuf, std::io::Error),

    #[error("cannot write video: {0}")]
    Video(#[from] rikka_video::Error),

    #[error("runtime directory has {available} MiB free but the draft needs {needed} MiB")]
    InsufficientSpace { available: u64, needed: u64 },

    #[error("{file} is not a current draft", file = .0.display())]
    UnknownDraft(PathBuf),

    #[error("draft was superseded by a newer one")]
    Superseded,

    #[error("screen capture failed: {0}")]
    Screen(#[from] rikka_capture::Error),

    #[error("cannot capture sound: {0}")]
    Audio(#[from] rikka_audio::Error),

    #[error("no encoder accepts the stream: {}", refusals(.0))]
    NoCodec(Vec<(VideoCodec, rikka_video::Error)>),

    #[error("cannot import the capture buffer: {0}")]
    Import(#[source] rikka_video::Error),

    #[error("cannot encode a frame: {0}")]
    Encode(#[source] rikka_video::Error),
}

fn refusals(refused: &[(VideoCodec, rikka_video::Error)]) -> String {
    refused
        .iter()
        .map(|(codec, error)| format!("{codec:?}: {error}"))
        .collect::<Vec<_>>()
        .join("; ")
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

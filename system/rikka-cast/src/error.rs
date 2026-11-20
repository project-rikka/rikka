use std::io;

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("no source selected")]
    NoSources,
    #[error("cannot spawn screencast thread: {0}")]
    Thread(#[source] io::Error),
    #[error("screencast thread exited before reporting its streams")]
    NoResponse,
    #[error("cannot connect to PipeWire: {0}")]
    Connect(#[source] pipewire::Error),
    #[error("cannot create PipeWire stream: {0}")]
    CreateStream(#[source] pipewire::Error),
    #[error("cannot connect PipeWire stream: {0}")]
    ConnectStream(#[source] pipewire::Error),
    #[error("cannot register stream listener")]
    AddListener,
    #[error("no video format to offer")]
    NoFormat,
    #[error("timed out waiting for PipeWire nodes")]
    NodeTimeout,
    #[error("cannot watch the Wayland connection: {0}")]
    WatchDisplay(#[source] io::Error),
    #[error("no output named \"{0}\"")]
    NoDisplay(String),
    #[error("no window with identifier \"{0}\"")]
    NoWindow(String),
    #[error(transparent)]
    Screen(#[from] rikka_capture::Error),
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

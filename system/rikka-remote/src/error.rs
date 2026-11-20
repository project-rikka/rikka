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

    #[error("no folder for the remote desktop's state")]
    NoFolder,

    #[error("no folder for the remote desktop's state: {0}")]
    Dirs(#[from] rikka_dirs::Error),

    #[error("cannot listen on UDP port {port}: {error}")]
    Listen {
        port: u16,
        error: rikka_remote_link::LinkError,
    },

    #[error("no remote desktop identity: {0}")]
    Identity(rikka_remote_link::LinkError),

    #[error("no account is signed in to this session")]
    NoUser,
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

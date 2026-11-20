use music_spotify_wire::LoginError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to reach the Spotify access point: {0}")]
    Unreachable(#[from] std::io::Error),

    #[error("the Spotify access point did not answer in time")]
    TimedOut,

    #[error("the Spotify access point closed the connection")]
    Disconnected,

    #[error("the Spotify access point stopped answering keepalives")]
    KeepAliveLost,

    #[error("the Spotify access point refused the login: {0}")]
    LoginFailed(LoginError),

    #[error("Spotify refused the audio key")]
    KeyDenied,

    #[error("unexpected Spotify access point packet: {0}")]
    Protocol(music_spotify_wire::Error),
}

impl From<music_spotify_wire::Error> for Error {
    fn from(error: music_spotify_wire::Error) -> Error {
        match error {
            music_spotify_wire::Error::LoginFailed(code) => Error::LoginFailed(code),
            error => Error::Protocol(error),
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

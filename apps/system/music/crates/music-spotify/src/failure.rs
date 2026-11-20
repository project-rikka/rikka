use music_core::Error;
use music_spotify_wire::LoginError;

pub(crate) fn api(error: music_spotify_api::Error) -> Error {
    use music_spotify_api::Error as Api;
    let message = error.to_string();
    match error {
        Api::Unreachable(_) | Api::Throttled(_) | Api::Challenge(_) => Error::Unreachable(message),
        Api::Status { status, .. } => Error::from_status(status, message),
        Api::TokenRefused { .. } => Error::Refused(message),
        Api::Storage(_) => Error::Missing(message),
        Api::Decode { .. } | Api::Malformed(_) => Error::Unreadable(message),
    }
}

pub(crate) fn session(error: music_spotify_session::Error) -> Error {
    use music_spotify_session::Error as Session;
    let message = error.to_string();
    match error {
        Session::Unreachable(_)
        | Session::TimedOut
        | Session::Disconnected
        | Session::KeepAliveLost
        | Session::LoginFailed(LoginError::TryAnotherAccessPoint) => Error::Unreachable(message),
        Session::LoginFailed(LoginError::PremiumAccountRequired) => Error::NotEntitled(message),
        Session::LoginFailed(_) | Session::KeyDenied => Error::Refused(message),
        Session::Protocol(_) => Error::Unreadable(message),
    }
}

pub(crate) fn unreadable(error: impl std::fmt::Display) -> Error {
    Error::Unreadable(error.to_string())
}

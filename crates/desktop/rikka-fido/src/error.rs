use std::io;

use crate::cbor;
use crate::status::CtapStatus;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("security key ceremony was cancelled")]
    Cancelled,

    #[error("security key ceremony timed out")]
    Timeout,

    #[error("security key was unplugged")]
    Disconnected,

    #[error("security key {0} cannot be opened: permission denied")]
    AccessDenied(String),

    #[error("security key I/O failed")]
    Io(#[source] io::Error),

    #[error("security key holds none of the allowed credentials")]
    NoCredentials,

    #[error("security key already holds a credential for this account")]
    CredentialExcluded,

    #[error("PIN is blocked; the security key must be reset")]
    PinBlocked,

    #[error("too many wrong PINs; unplug the security key and plug it in again")]
    PinAuthBlocked,

    #[error("security key has no PIN set")]
    PinNotSet,

    #[error("security key already has a PIN")]
    PinAlreadySet,

    #[error("security key does not support a PIN")]
    PinUnsupported,

    #[error("PIN is wrong; {retries} attempts remain")]
    WrongPin { retries: u32 },

    #[error("new PIN needs at least {min_length} characters and at most 63 bytes")]
    PinPolicyViolation { min_length: usize },

    #[error("security key's built-in user verification is blocked")]
    UvBlocked,

    #[error("security key cannot verify the user")]
    UserVerificationUnavailable,

    #[error("security key cannot store discoverable credentials")]
    ResidentKeyUnavailable,

    #[error("security key supports none of the requested algorithms")]
    UnsupportedAlgorithm,

    #[error("security key does not support the {0} extension")]
    ExtensionUnsupported(&'static str),

    #[error("request is too large for the security key")]
    RequestTooLarge,

    #[error("{0} is not a secure web origin")]
    InvalidOrigin(String),

    #[error("relying party {rp_id} does not belong to the origin {origin}")]
    RelyingPartyMismatch { rp_id: String, origin: String },

    #[error("AppID {0} does not belong to the relying party")]
    InvalidAppId(String),

    #[error("invalid request: {0}")]
    InvalidRequest(&'static str),

    #[error("security key rejected the request: {0}")]
    Ctap(CtapStatus),

    #[error("security key's HID layer reported error {0:#04x}")]
    Hid(u8),

    #[error("U2F security key returned status {0:#06x}")]
    U2f(u16),

    #[error("malformed security key response: {0}")]
    Protocol(&'static str),

    #[error("malformed CBOR from security key")]
    Cbor(#[from] cbor::Error),

    #[error("failed to spawn security key thread")]
    Spawn(#[source] io::Error),

    #[error("security key thread exited unexpectedly")]
    Aborted,
}

impl Error {
    pub(crate) fn io(error: io::Error) -> Error {
        const DISCONNECTED_ERRNOS: [i32; 4] = [libc::ENODEV, libc::ENXIO, libc::EIO, libc::EPIPE];

        let disconnected = error
            .raw_os_error()
            .is_some_and(|code| DISCONNECTED_ERRNOS.contains(&code))
            || matches!(
                error.kind(),
                io::ErrorKind::NotFound
                    | io::ErrorKind::NotConnected
                    | io::ErrorKind::BrokenPipe
                    | io::ErrorKind::UnexpectedEof
            );

        if disconnected {
            Error::Disconnected
        } else {
            Error::Io(error)
        }
    }

    pub(crate) fn is_stop(&self) -> bool {
        matches!(self, Error::Cancelled | Error::Timeout)
    }
}

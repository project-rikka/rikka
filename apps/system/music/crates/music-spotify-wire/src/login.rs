use std::fmt;

use prost::Message as _;

use crate::messages::{ClientResponseEncrypted, LoginCredentials, SystemInfo};

const STORED_SPOTIFY_CREDENTIALS: i32 = 1;
const SPOTIFY_TOKEN: i32 = 3;
const OS_LINUX: i32 = 5;
const CLIENT_NAME: &str = "rikka-music";

#[derive(Clone, PartialEq, Eq)]
pub enum Credentials {
    AccessToken(String),
    Stored(StoredCredentials),
}

#[derive(Clone, PartialEq, Eq)]
pub struct StoredCredentials {
    pub username: String,
    pub auth_data: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LoginError {
    ProtocolError,
    TryAnotherAccessPoint,
    BadConnectionId,
    TravelRestriction,
    PremiumAccountRequired,
    BadCredentials,
    CouldNotValidateCredentials,
    AccountExists,
    ExtraVerificationRequired,
    InvalidAppKey,
    ApplicationBanned,
    Unknown(i32),
}

impl Credentials {
    pub(crate) fn encode(&self, device_id: &str) -> Vec<u8> {
        let (username, kind, auth_data) = match self {
            Credentials::AccessToken(token) => (None, SPOTIFY_TOKEN, token.as_bytes().to_vec()),
            Credentials::Stored(stored) => (
                Some(stored.username.clone()),
                STORED_SPOTIFY_CREDENTIALS,
                stored.auth_data.clone(),
            ),
        };
        ClientResponseEncrypted {
            login_credentials: Some(LoginCredentials {
                username,
                typ: kind,
                auth_data: Some(auth_data),
            }),
            system_info: Some(SystemInfo {
                cpu_family: cpu_family(),
                os: OS_LINUX,
                system_information_string: Some(CLIENT_NAME.to_owned()),
                device_id: Some(device_id.to_owned()),
            }),
            version_string: Some(CLIENT_NAME.to_owned()),
        }
        .encode_to_vec()
    }
}

impl LoginError {
    #[must_use]
    pub fn from_code(code: i32) -> LoginError {
        match code {
            0x00 => LoginError::ProtocolError,
            0x02 => LoginError::TryAnotherAccessPoint,
            0x05 => LoginError::BadConnectionId,
            0x09 => LoginError::TravelRestriction,
            0x0b => LoginError::PremiumAccountRequired,
            0x0c => LoginError::BadCredentials,
            0x0d => LoginError::CouldNotValidateCredentials,
            0x0e => LoginError::AccountExists,
            0x0f => LoginError::ExtraVerificationRequired,
            0x10 => LoginError::InvalidAppKey,
            0x11 => LoginError::ApplicationBanned,
            other => LoginError::Unknown(other),
        }
    }
}

impl fmt::Display for LoginError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoginError::ProtocolError => formatter.write_str("protocol error"),
            LoginError::TryAnotherAccessPoint => formatter.write_str("try another access point"),
            LoginError::BadConnectionId => formatter.write_str("bad connection id"),
            LoginError::TravelRestriction => formatter.write_str("travel restriction"),
            LoginError::PremiumAccountRequired => formatter.write_str("premium account required"),
            LoginError::BadCredentials => formatter.write_str("bad credentials"),
            LoginError::CouldNotValidateCredentials => {
                formatter.write_str("could not validate credentials")
            }
            LoginError::AccountExists => formatter.write_str("account exists"),
            LoginError::ExtraVerificationRequired => {
                formatter.write_str("extra verification required")
            }
            LoginError::InvalidAppKey => formatter.write_str("invalid app key"),
            LoginError::ApplicationBanned => formatter.write_str("application banned"),
            LoginError::Unknown(code) => write!(formatter, "error code {code}"),
        }
    }
}

impl fmt::Debug for Credentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Credentials::AccessToken(_) => formatter.write_str("AccessToken(..)"),
            Credentials::Stored(stored) => formatter.debug_tuple("Stored").field(stored).finish(),
        }
    }
}

impl fmt::Debug for StoredCredentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StoredCredentials")
            .field("username", &self.username)
            .finish_non_exhaustive()
    }
}

fn cpu_family() -> i32 {
    const CPU_UNKNOWN: i32 = 0x00;
    const CPU_X86: i32 = 0x01;
    const CPU_X86_64: i32 = 0x02;
    const CPU_ARM: i32 = 0x05;
    match std::env::consts::ARCH {
        "x86" => CPU_X86,
        "x86_64" => CPU_X86_64,
        "arm" | "aarch64" => CPU_ARM,
        _ => CPU_UNKNOWN,
    }
}

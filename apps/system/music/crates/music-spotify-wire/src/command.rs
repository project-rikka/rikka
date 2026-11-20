#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Command {
    Ping,
    RequestKey,
    AesKey,
    AesKeyError,
    CountryCode,
    Pong,
    PongAck,
    ProductInfo,
    Login,
    ApWelcome,
    AuthFailure,
    Other(u8),
}

impl Command {
    #[must_use]
    pub fn from_byte(byte: u8) -> Command {
        match byte {
            0x04 => Command::Ping,
            0x0c => Command::RequestKey,
            0x0d => Command::AesKey,
            0x0e => Command::AesKeyError,
            0x1b => Command::CountryCode,
            0x49 => Command::Pong,
            0x4a => Command::PongAck,
            0x50 => Command::ProductInfo,
            0xab => Command::Login,
            0xac => Command::ApWelcome,
            0xad => Command::AuthFailure,
            other => Command::Other(other),
        }
    }

    #[must_use]
    pub fn to_byte(self) -> u8 {
        match self {
            Command::Ping => 0x04,
            Command::RequestKey => 0x0c,
            Command::AesKey => 0x0d,
            Command::AesKeyError => 0x0e,
            Command::CountryCode => 0x1b,
            Command::Pong => 0x49,
            Command::PongAck => 0x4a,
            Command::ProductInfo => 0x50,
            Command::Login => 0xab,
            Command::ApWelcome => 0xac,
            Command::AuthFailure => 0xad,
            Command::Other(byte) => byte,
        }
    }
}

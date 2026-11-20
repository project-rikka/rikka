use prost::Message as _;

use crate::command::Command;
use crate::error::{Error, Result};
use crate::login::{LoginError, StoredCredentials};
use crate::messages::{ApLoginFailed, ApWelcome};
use crate::packet::Packet;
use crate::product::Product;

const SEQUENCE_BYTES: usize = 4;

pub const AUDIO_KEY_LEN: usize = 16;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Incoming {
    Ping {
        server_time: u32,
    },
    PongAck,
    CountryCode(String),
    ProductInfo(Product),
    AudioKey {
        sequence: u32,
        key: [u8; AUDIO_KEY_LEN],
    },
    AudioKeyDenied {
        sequence: u32,
    },
    Welcome(StoredCredentials),
    LoginFailed(LoginError),
    Other(Command),
}

impl Incoming {
    pub fn read(packet: &Packet) -> Result<Incoming> {
        let payload = packet.payload.as_slice();
        Ok(match packet.command {
            Command::Ping => Incoming::Ping {
                server_time: u32::from_be_bytes(leading(payload, "ping")?),
            },
            Command::PongAck => Incoming::PongAck,
            Command::CountryCode => Incoming::CountryCode(
                String::from_utf8(payload.to_vec())
                    .map_err(|_| Error::Malformed("country code"))?,
            ),
            Command::ProductInfo => Incoming::ProductInfo(Product::parse(
                std::str::from_utf8(payload).map_err(|_| Error::Malformed("product info"))?,
            )?),
            Command::AesKey => {
                let sequence = u32::from_be_bytes(leading(payload, "audio key")?);
                let key = payload[SEQUENCE_BYTES..]
                    .get(..AUDIO_KEY_LEN)
                    .and_then(|key| key.try_into().ok())
                    .ok_or(Error::Malformed("audio key"))?;
                Incoming::AudioKey { sequence, key }
            }
            Command::AesKeyError => Incoming::AudioKeyDenied {
                sequence: u32::from_be_bytes(leading(payload, "audio key error")?),
            },
            Command::ApWelcome => {
                let welcome = ApWelcome::decode(payload).map_err(Error::decode("welcome"))?;
                Incoming::Welcome(StoredCredentials {
                    username: welcome
                        .canonical_username
                        .ok_or(Error::Malformed("welcome username"))?,
                    auth_data: welcome
                        .reusable_auth_credentials
                        .ok_or(Error::Malformed("welcome credentials"))?,
                })
            }
            Command::AuthFailure => {
                let failed =
                    ApLoginFailed::decode(payload).map_err(Error::decode("login failure"))?;
                Incoming::LoginFailed(LoginError::from_code(
                    failed.error_code.ok_or(Error::Malformed("login failure"))?,
                ))
            }
            other => Incoming::Other(other),
        })
    }
}

fn leading(payload: &[u8], subject: &'static str) -> Result<[u8; SEQUENCE_BYTES]> {
    payload
        .get(..SEQUENCE_BYTES)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(Error::Malformed(subject))
}

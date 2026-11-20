use crate::command::Command;
use crate::file_id::FileId;
use crate::login::Credentials;
use crate::spotify_id::SpotifyId;

const PONG: [u8; 4] = [0; 4];
const KEY_REQUEST_TRAILER: [u8; 2] = [0; 2];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Packet {
    pub command: Command,
    pub payload: Vec<u8>,
}

impl Packet {
    #[must_use]
    pub fn new(command: Command, payload: Vec<u8>) -> Packet {
        Packet { command, payload }
    }

    #[must_use]
    pub fn pong() -> Packet {
        Packet::new(Command::Pong, PONG.to_vec())
    }

    #[must_use]
    pub fn login(credentials: &Credentials, device_id: &str) -> Packet {
        Packet::new(Command::Login, credentials.encode(device_id))
    }

    #[must_use]
    pub fn audio_key_request(sequence: u32, track: SpotifyId, file: FileId) -> Packet {
        let mut payload = Vec::with_capacity(FileId::LEN + SpotifyId::GID_LEN + 6);
        payload.extend_from_slice(file.as_bytes());
        payload.extend_from_slice(&track.to_gid());
        payload.extend_from_slice(&sequence.to_be_bytes());
        payload.extend_from_slice(&KEY_REQUEST_TRAILER);
        Packet::new(Command::RequestKey, payload)
    }
}

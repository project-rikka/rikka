use crate::command::Command;
use crate::error::{Error, Result};
use crate::handshake::KEY_LEN;
use crate::packet::Packet;
use crate::shannon::Shannon;

const HEADER_LEN: usize = 3;
const MAC_LEN: usize = 4;

#[derive(Debug)]
pub struct Encoder {
    cipher: Shannon,
    nonce: u32,
}

#[derive(Debug)]
pub struct Decoder {
    cipher: Shannon,
    nonce: u32,
    buffer: Vec<u8>,
    header: Option<(Command, usize)>,
}

impl Encoder {
    #[must_use]
    pub fn new(key: &[u8; KEY_LEN]) -> Encoder {
        Encoder {
            cipher: Shannon::new(key),
            nonce: 0,
        }
    }

    pub fn encode(&mut self, packet: &Packet) -> Result<Vec<u8>> {
        let length = u16::try_from(packet.payload.len())
            .map_err(|_| Error::Oversized(packet.payload.len()))?;
        let mut frame = Vec::with_capacity(HEADER_LEN + packet.payload.len() + MAC_LEN);
        frame.push(packet.command.to_byte());
        frame.extend_from_slice(&length.to_be_bytes());
        frame.extend_from_slice(&packet.payload);
        self.cipher.set_nonce(self.nonce);
        self.nonce = self.nonce.wrapping_add(1);
        self.cipher.encrypt(&mut frame);
        let mut mac = [0u8; MAC_LEN];
        self.cipher.finish(&mut mac);
        frame.extend_from_slice(&mac);
        Ok(frame)
    }
}

impl Decoder {
    #[must_use]
    pub fn new(key: &[u8; KEY_LEN]) -> Decoder {
        Decoder {
            cipher: Shannon::new(key),
            nonce: 0,
            buffer: Vec::new(),
            header: None,
        }
    }

    pub fn push(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    pub fn next_packet(&mut self) -> Result<Option<Packet>> {
        let (command, length) = match self.header {
            Some(header) => header,
            None => {
                if self.buffer.len() < HEADER_LEN {
                    return Ok(None);
                }
                let mut header: [u8; HEADER_LEN] = self.buffer[..HEADER_LEN]
                    .try_into()
                    .expect("the buffer holds a whole header");
                self.buffer.drain(..HEADER_LEN);
                self.cipher.set_nonce(self.nonce);
                self.nonce = self.nonce.wrapping_add(1);
                self.cipher.decrypt(&mut header);
                let header = (
                    Command::from_byte(header[0]),
                    usize::from(u16::from_be_bytes([header[1], header[2]])),
                );
                self.header = Some(header);
                header
            }
        };
        if self.buffer.len() < length + MAC_LEN {
            return Ok(None);
        }
        self.header = None;
        let mut payload: Vec<u8> = self.buffer.drain(..length).collect();
        let received: Vec<u8> = self.buffer.drain(..MAC_LEN).collect();
        self.cipher.decrypt(&mut payload);
        let mut expected = [0u8; MAC_LEN];
        self.cipher.finish(&mut expected);
        if expected.as_slice() != received.as_slice() {
            return Err(Error::Mac);
        }
        Ok(Some(Packet::new(command, payload)))
    }
}

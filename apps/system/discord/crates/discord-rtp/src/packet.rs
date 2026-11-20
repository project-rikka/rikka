use crate::cipher::Cipher;
use crate::rtp::{self, Header};

const RTCP_CLEAR: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opened {
    pub header: Header,
    pub payload: Vec<u8>,
}

impl Cipher {
    #[must_use]
    pub fn seal_rtp(&self, packet: &[u8], counter: u32) -> Option<Vec<u8>> {
        let header = Header::parse(packet)?;
        let (clear, body) = packet.split_at(header.clear);
        self.seal(clear, body, counter)
    }

    #[must_use]
    pub fn open_rtp(&self, packet: &[u8]) -> Option<Opened> {
        let header = Header::parse(packet)?;
        let (clear, sealed) = packet.split_at(header.clear);
        let mut payload = self.decrypt(clear, sealed)?;
        if payload.len() < header.extension {
            return None;
        }
        payload.drain(..header.extension);
        if header.padded {
            let padding = usize::from(*payload.last()?);
            if padding == 0 || padding > payload.len() {
                return None;
            }
            payload.truncate(payload.len() - padding);
        }
        Some(Opened { header, payload })
    }

    #[must_use]
    pub fn seal_rtcp(&self, rtcp: &[u8], counter: u32) -> Option<Vec<u8>> {
        if rtcp.len() < RTCP_CLEAR {
            return None;
        }
        let (clear, body) = rtcp.split_at(RTCP_CLEAR);
        self.seal(clear, body, counter)
    }

    #[must_use]
    pub fn open_rtcp(&self, packet: &[u8]) -> Option<Vec<u8>> {
        if !rtp::is_rtcp(packet) {
            return None;
        }
        let (clear, sealed) = packet.split_at(RTCP_CLEAR);
        let body = self.decrypt(clear, sealed)?;
        let mut whole = Vec::with_capacity(clear.len() + body.len());
        whole.extend_from_slice(clear);
        whole.extend_from_slice(&body);
        Some(whole)
    }

    fn seal(&self, clear: &[u8], body: &[u8], counter: u32) -> Option<Vec<u8>> {
        let sealed = self.encrypt(clear, body, counter)?;
        let mut packet = Vec::with_capacity(clear.len() + sealed.len());
        packet.extend_from_slice(clear);
        packet.extend_from_slice(&sealed);
        Some(packet)
    }
}

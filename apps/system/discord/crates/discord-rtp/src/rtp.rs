pub const HEADER_LEN: usize = 12;

const VERSION: u8 = 2;

const CSRC_LEN: usize = 4;

const EXTENSION_PREAMBLE: usize = 4;

const MARKER: u8 = 0x80;

const EXTENDED: u8 = 0x10;

const PADDED: u8 = 0x20;

pub(crate) const ONE_BYTE_PROFILE: [u8; 2] = [0xBE, 0xDE];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub padded: bool,
    pub marker: bool,
    pub payload_type: u8,
    pub sequence: u16,
    pub timestamp: u32,
    pub ssrc: u32,
    pub clear: usize,
    pub extension: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fields {
    pub marker: bool,
    pub payload_type: u8,
    pub sequence: u16,
    pub timestamp: u32,
    pub ssrc: u32,
}

impl Fields {
    #[must_use]
    pub fn header(self, extended: bool) -> [u8; HEADER_LEN] {
        let mut header = [0; HEADER_LEN];
        header[0] = VERSION << 6 | if extended { EXTENDED } else { 0 };
        header[1] = self.payload_type | if self.marker { MARKER } else { 0 };
        header[2..4].copy_from_slice(&self.sequence.to_be_bytes());
        header[4..8].copy_from_slice(&self.timestamp.to_be_bytes());
        header[8..12].copy_from_slice(&self.ssrc.to_be_bytes());
        header
    }
}

impl Header {
    #[must_use]
    pub fn opus(sequence: u16, timestamp: u32, ssrc: u32, marker: bool) -> [u8; HEADER_LEN] {
        Fields {
            marker,
            payload_type: crate::codec::OPUS_PAYLOAD_TYPE,
            sequence,
            timestamp,
            ssrc,
        }
        .header(false)
    }

    #[must_use]
    pub fn parse(packet: &[u8]) -> Option<Header> {
        let fixed: &[u8; HEADER_LEN] = packet.get(..HEADER_LEN)?.try_into().ok()?;
        if fixed[0] >> 6 != VERSION {
            return None;
        }
        let csrcs = usize::from(fixed[0] & 0x0F);
        let extended = fixed[0] & EXTENDED != 0;
        let mut clear = HEADER_LEN + csrcs * CSRC_LEN;
        let mut extension = 0;
        if extended {
            let preamble = packet.get(clear..clear + EXTENSION_PREAMBLE)?;
            extension = usize::from(u16::from_be_bytes([preamble[2], preamble[3]])) * 4;
            clear += EXTENSION_PREAMBLE;
        }
        if packet.len() < clear {
            return None;
        }
        Some(Header {
            padded: fixed[0] & PADDED != 0,
            marker: fixed[1] & MARKER != 0,
            payload_type: fixed[1] & 0x7F,
            sequence: u16::from_be_bytes([fixed[2], fixed[3]]),
            timestamp: u32::from_be_bytes([fixed[4], fixed[5], fixed[6], fixed[7]]),
            ssrc: u32::from_be_bytes([fixed[8], fixed[9], fixed[10], fixed[11]]),
            clear,
            extension,
        })
    }
}

#[must_use]
pub fn is_rtcp(packet: &[u8]) -> bool {
    packet.len() >= 8 && packet[0] >> 6 == VERSION && (192..=223).contains(&packet[1])
}

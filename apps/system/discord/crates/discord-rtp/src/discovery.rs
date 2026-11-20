pub const DISCOVERY_LEN: usize = 74;

const REQUEST: u16 = 1;

const RESPONSE: u16 = 2;

const BODY_LEN: u16 = 70;

const ADDRESS: std::ops::Range<usize> = 8..72;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discovered {
    pub address: String,
    pub port: u16,
}

impl Discovered {
    #[must_use]
    pub fn request(ssrc: u32) -> [u8; DISCOVERY_LEN] {
        let mut packet = [0; DISCOVERY_LEN];
        packet[0..2].copy_from_slice(&REQUEST.to_be_bytes());
        packet[2..4].copy_from_slice(&BODY_LEN.to_be_bytes());
        packet[4..8].copy_from_slice(&ssrc.to_be_bytes());
        packet
    }

    #[must_use]
    pub fn from_response(packet: &[u8], ssrc: u32) -> Option<Discovered> {
        let packet: &[u8; DISCOVERY_LEN] = packet.try_into().ok()?;
        if u16::from_be_bytes([packet[0], packet[1]]) != RESPONSE
            || u16::from_be_bytes([packet[2], packet[3]]) != BODY_LEN
            || u32::from_be_bytes([packet[4], packet[5], packet[6], packet[7]]) != ssrc
        {
            return None;
        }
        let field = &packet[ADDRESS];
        let end = field.iter().position(|&byte| byte == 0)?;
        let address = std::str::from_utf8(&field[..end]).ok()?;
        if address.is_empty() {
            return None;
        }
        Some(Discovered {
            address: address.to_owned(),
            port: u16::from_be_bytes([packet[72], packet[73]]),
        })
    }
}

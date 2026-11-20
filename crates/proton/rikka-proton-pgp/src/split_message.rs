use crate::session_key::SessionKey;

#[derive(Debug, Clone)]
pub struct SplitMessage {
    key_packets: Vec<u8>,
    data_packet: Vec<u8>,
    session_key: SessionKey,
}

impl SplitMessage {
    #[must_use]
    pub fn key_packets(&self) -> &[u8] {
        &self.key_packets
    }

    #[must_use]
    pub fn data_packet(&self) -> &[u8] {
        &self.data_packet
    }

    #[must_use]
    pub fn session_key(&self) -> &SessionKey {
        &self.session_key
    }

    #[must_use]
    pub fn into_parts(self) -> (Vec<u8>, Vec<u8>, SessionKey) {
        (self.key_packets, self.data_packet, self.session_key)
    }

    pub(crate) fn new(key_packets: Vec<u8>, data_packet: Vec<u8>, session_key: SessionKey) -> Self {
        SplitMessage {
            key_packets,
            data_packet,
            session_key,
        }
    }
}

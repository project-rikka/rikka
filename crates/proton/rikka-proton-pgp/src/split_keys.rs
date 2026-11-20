use crate::session_key::SessionKey;

#[derive(Debug, Clone)]
pub struct SplitKeys {
    key_packets: Vec<u8>,
    session_key: SessionKey,
}

impl SplitKeys {
    #[must_use]
    pub fn key_packets(&self) -> &[u8] {
        &self.key_packets
    }

    #[must_use]
    pub fn session_key(&self) -> &SessionKey {
        &self.session_key
    }

    #[must_use]
    pub fn into_parts(self) -> (Vec<u8>, SessionKey) {
        (self.key_packets, self.session_key)
    }

    pub(crate) fn new(key_packets: Vec<u8>, session_key: SessionKey) -> Self {
        SplitKeys {
            key_packets,
            session_key,
        }
    }
}

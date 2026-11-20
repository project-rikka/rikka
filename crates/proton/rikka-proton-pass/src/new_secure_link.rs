use std::time::Duration;

#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewSecureLink {
    expires_in: Duration,
    max_reads: Option<u32>,
}

impl NewSecureLink {
    pub fn new(expires_in: Duration) -> NewSecureLink {
        NewSecureLink {
            expires_in,
            max_reads: None,
        }
    }

    pub fn max_reads(mut self, reads: u32) -> NewSecureLink {
        self.max_reads = Some(reads);
        self
    }

    pub(crate) fn expires_in_seconds(&self) -> u64 {
        self.expires_in.as_secs()
    }

    pub(crate) fn read_limit(&self) -> Option<u32> {
        self.max_reads
    }
}

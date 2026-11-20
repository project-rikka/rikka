use crate::error::{Error, Result};

const VERSION: [u8; 4] = [1, 0, 0, 0];
const RECORD_LEN: usize = 6;
const ENABLED: u8 = 1;
const VISIBLE: u8 = 1 << 1;
const AUTOCONNECTABLE: u8 = 1 << 2;
const MAX_LOAD: u8 = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryStatus {
    records: Vec<u8>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ServerStatus {
    flags: u8,
    load: u8,
    partial_score: f32,
}

impl BinaryStatus {
    pub fn parse(bytes: impl AsRef<[u8]>) -> Result<BinaryStatus> {
        let bytes = bytes.as_ref();
        let header = bytes.get(..VERSION.len()).ok_or(Error::MalformedStatus {
            reason: "missing version header",
        })?;
        if header != VERSION {
            return Err(Error::MalformedStatus {
                reason: "unsupported version",
            });
        }
        let records = &bytes[VERSION.len()..];
        if records.len() % RECORD_LEN != 0 {
            return Err(Error::MalformedStatus {
                reason: "truncated server record",
            });
        }
        Ok(BinaryStatus {
            records: records.to_vec(),
        })
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len() / RECORD_LEN
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    #[must_use]
    pub fn get(&self, index: u32) -> Option<ServerStatus> {
        let start = usize::try_from(index).ok()?.checked_mul(RECORD_LEN)?;
        let record = self.records.get(start..start + RECORD_LEN)?;
        ServerStatus::from_record(record)
    }

    pub(crate) fn get_or_unknown(&self, index: u32) -> ServerStatus {
        self.get(index).unwrap_or_default()
    }
}

impl ServerStatus {
    fn from_record(record: &[u8]) -> Option<ServerStatus> {
        let partial_score = f32::from_le_bytes([record[2], record[3], record[4], record[5]]);
        let status = ServerStatus {
            flags: record[0],
            load: record[1],
            partial_score,
        };
        (status.load <= MAX_LOAD && (0.0..=1.0).contains(&status.partial_score)).then_some(status)
    }

    #[must_use]
    pub fn is_enabled(self) -> bool {
        self.flags & ENABLED != 0
    }

    #[must_use]
    pub fn is_visible(self) -> bool {
        self.flags & VISIBLE != 0
    }

    #[must_use]
    pub fn is_autoconnectable(self) -> bool {
        self.flags & AUTOCONNECTABLE != 0
    }

    #[must_use]
    pub fn load(self) -> u8 {
        self.load
    }

    #[must_use]
    pub fn partial_score(self) -> f32 {
        self.partial_score
    }
}

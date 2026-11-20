use music_core::Result;

const RANGE: u64 = 1024 * 1024;
const LARGEST_UNSIZED: u64 = 256 * 1024 * 1024;

type Frame = Box<dyn Fn(u64) -> Result<Vec<u8>> + Send + Sync>;

#[must_use]
#[derive(Default)]
pub struct Plan {
    pub(crate) lead: Vec<u8>,
    pub(crate) pieces: Vec<Piece>,
    pub(crate) open: bool,
    pub(crate) headers: Vec<(&'static str, String)>,
    pub(crate) frame: Option<Frame>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Piece {
    pub(crate) url: String,
    pub(crate) from: u64,
    pub(crate) length: Option<u64>,
    pub(crate) ranged: bool,
}

impl Plan {
    pub fn new() -> Plan {
        Plan::default()
    }

    pub fn file(url: impl Into<String>, length: Option<u64>) -> Plan {
        let url = url.into();
        let open = length.is_none();
        let length = length.unwrap_or(LARGEST_UNSIZED);
        let pieces = (0..length.div_ceil(RANGE))
            .map(|index| {
                let from = index * RANGE;
                Piece {
                    url: url.clone(),
                    from,
                    length: Some(RANGE.min(length - from)),
                    ranged: true,
                }
            })
            .collect();
        Plan {
            pieces,
            open,
            ..Plan::default()
        }
    }

    pub fn piece(mut self, url: impl Into<String>, length: Option<u64>) -> Plan {
        self.pieces.push(Piece {
            url: url.into(),
            from: 0,
            length,
            ranged: false,
        });
        self
    }

    pub fn lead(mut self, bytes: impl Into<Vec<u8>>) -> Plan {
        self.lead = bytes.into();
        self
    }

    pub fn header(mut self, name: &'static str, value: impl Into<String>) -> Plan {
        self.headers.push((name, value.into()));
        self
    }

    pub fn framed(
        mut self,
        frame: impl Fn(u64) -> Result<Vec<u8>> + Send + Sync + 'static,
    ) -> Plan {
        self.frame = Some(Box::new(frame));
        self
    }

    pub(crate) fn total(&self) -> Option<u64> {
        if self.open || self.frame.is_some() {
            return None;
        }
        self.pieces
            .iter()
            .map(|piece| piece.length)
            .sum::<Option<u64>>()
            .map(|length| length + self.lead.len() as u64)
    }
}

impl std::fmt::Debug for Plan {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Plan")
            .field("lead", &self.lead.len())
            .field("pieces", &self.pieces.len())
            .field("open", &self.open)
            .field("framed", &self.frame.is_some())
            .finish_non_exhaustive()
    }
}

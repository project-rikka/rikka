use x11rb_protocol::protocol::xproto::Atom;

const HEADER: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Delivery {
    Whole,
    Begin(usize),
    Chunk,
}

#[derive(Debug)]
pub(crate) struct Outgoing {
    buffer: Vec<u8>,
    chunk: usize,
    ended: bool,
    incremental: bool,
    awaiting_delete: bool,
    finished: bool,
}

impl Outgoing {
    pub(crate) fn new(chunk: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(chunk),
            chunk,
            ended: false,
            incremental: false,
            awaiting_delete: false,
            finished: false,
        }
    }

    pub(crate) fn wants_input(&self) -> bool {
        !self.ended && self.buffer.len() < self.chunk
    }

    pub(crate) fn finished(&self) -> bool {
        self.finished
    }

    pub(crate) fn space(&mut self) -> &mut Vec<u8> {
        &mut self.buffer
    }

    pub(crate) fn data(&self) -> &[u8] {
        &self.buffer
    }

    pub(crate) fn delivered(&mut self) {
        self.buffer.clear();
    }

    pub(crate) fn received(&mut self, length: usize) -> Option<Delivery> {
        if length == 0 {
            self.ended = true;
        }
        self.advance()
    }

    pub(crate) fn deleted(&mut self) -> Option<Delivery> {
        if !self.awaiting_delete {
            return None;
        }
        self.awaiting_delete = false;
        self.advance()
    }

    fn advance(&mut self) -> Option<Delivery> {
        if self.finished || self.awaiting_delete {
            return None;
        }
        if !self.incremental {
            if self.ended {
                self.finished = true;
                return Some(Delivery::Whole);
            }
            if self.buffer.len() >= self.chunk {
                self.incremental = true;
                self.awaiting_delete = true;
                return Some(Delivery::Begin(self.buffer.len()));
            }
            return None;
        }
        if self.buffer.len() < self.chunk && !self.ended {
            return None;
        }
        if self.buffer.is_empty() {
            self.finished = true;
        } else {
            self.awaiting_delete = true;
        }
        Some(Delivery::Chunk)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Next {
    Write,
    Fetch { offset: u32 },
    Finish,
    Wait,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Unknown,
    Whole,
    Incremental,
}

#[derive(Debug)]
pub(crate) struct Incoming {
    chunk: Vec<u8>,
    start: usize,
    end: usize,
    offset: u32,
    more: bool,
    mode: Mode,
    fetching: bool,
    announced: bool,
    complete: bool,
}

impl Default for Incoming {
    fn default() -> Self {
        Self {
            chunk: Vec::new(),
            start: 0,
            end: 0,
            offset: 0,
            more: false,
            mode: Mode::Unknown,
            fetching: false,
            announced: false,
            complete: false,
        }
    }
}

impl Incoming {
    pub(crate) fn arrived(&mut self, packet: Vec<u8>, incremental: Atom) {
        self.fetching = false;
        self.announced = false;
        let word = |at: usize| {
            packet.get(at..at + 4).map_or(0, |bytes| {
                u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
            })
        };
        let format = usize::from(packet.get(1).copied().unwrap_or_default() / 8);
        let type_ = word(8);
        let after = word(12);
        let length = (word(16) as usize * format).min(packet.len().saturating_sub(HEADER));
        if self.mode == Mode::Unknown {
            if type_ == incremental {
                self.mode = Mode::Incremental;
                self.more = false;
                return;
            }
            self.mode = Mode::Whole;
        }
        if self.mode == Mode::Incremental && self.offset == 0 && length == 0 && after == 0 {
            self.complete = true;
            return;
        }
        self.offset += u32::try_from(length / 4).unwrap_or(u32::MAX);
        self.more = after > 0;
        self.chunk = packet;
        self.start = HEADER;
        self.end = HEADER + length;
    }

    pub(crate) fn announced(&mut self) {
        if self.mode == Mode::Incremental {
            self.announced = true;
        }
    }

    pub(crate) fn pending(&self) -> &[u8] {
        &self.chunk[self.start..self.end]
    }

    pub(crate) fn wrote(&mut self, length: usize) {
        self.start = (self.start + length).min(self.end);
    }

    pub(crate) fn next(&mut self) -> Next {
        if self.start < self.end {
            return Next::Write;
        }
        if self.complete {
            return Next::Finish;
        }
        if self.fetching {
            return Next::Wait;
        }
        let offset = match self.mode {
            Mode::Unknown => 0,
            _ if self.more => self.offset,
            Mode::Whole => {
                self.complete = true;
                return Next::Finish;
            }
            Mode::Incremental if self.announced => {
                self.announced = false;
                self.offset = 0;
                0
            }
            Mode::Incremental => return Next::Wait,
        };
        self.chunk = Vec::new();
        self.start = 0;
        self.end = 0;
        self.fetching = true;
        Next::Fetch { offset }
    }
}

pub const DISPLAY_ID: u32 = 1;
pub const CLIENT_ID_MAX: u32 = 0xFEFF_FFFF;
pub const SERVER_ID_START: u32 = 0xFF00_0000;
pub const MAX_MESSAGE_SIZE: usize = 4096;
pub const MAX_FDS_PER_MESSAGE: usize = 28;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ClientId(u64);

impl ClientId {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Default)]
pub struct IdPool {
    issued: u32,
    free: Vec<u32>,
}

impl IdPool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn allocate(&mut self) -> Option<u32> {
        if let Some(id) = self.free.pop() {
            return Some(id);
        }
        let id = SERVER_ID_START.checked_add(self.issued)?;
        self.issued += 1;
        Some(id)
    }

    pub fn release(&mut self, id: u32) {
        if id >= SERVER_ID_START && id - SERVER_ID_START < self.issued {
            self.free.push(id);
        }
    }

    pub fn issued(&self) -> u32 {
        self.issued
    }
}

pub fn is_client_id(id: u32) -> bool {
    id != 0 && id <= CLIENT_ID_MAX
}

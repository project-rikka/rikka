const CAPACITY: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StrikeKey {
    pub(crate) font: usize,
    pub(crate) len: usize,
    pub(crate) face: u32,
    pub(crate) glyph: u16,
    pub(crate) ppem: u16,
    pub(crate) table: StrikeTable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StrikeTable {
    Cbdt,
    Sbix,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Decoded {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) left: f32,
    pub(crate) top: f32,
    pub(crate) strike_ppem_x: f32,
    pub(crate) strike_ppem_y: f32,
}

#[derive(Debug)]
struct Entry {
    key: StrikeKey,
    used: u64,
    decoded: Decoded,
    pixels: Vec<u8>,
}

#[derive(Debug, Default)]
pub(crate) struct DecodedStrikes {
    entries: Vec<Entry>,
    clock: u64,
}

impl DecodedStrikes {
    pub(crate) fn get_or_decode(
        &mut self,
        key: StrikeKey,
        decode: impl FnOnce(&mut Vec<u8>) -> Option<Decoded>,
    ) -> Option<(Decoded, &[u8])> {
        self.clock += 1;
        if let Some(at) = self.entries.iter().position(|entry| entry.key == key) {
            let entry = &mut self.entries[at];
            entry.used = self.clock;
            return Some((entry.decoded, &entry.pixels));
        }
        let at = if self.entries.len() < CAPACITY {
            self.entries.push(Entry {
                key,
                used: 0,
                decoded: Decoded::default(),
                pixels: Vec::new(),
            });
            self.entries.len() - 1
        } else {
            self.entries
                .iter()
                .enumerate()
                .min_by_key(|(_, entry)| entry.used)
                .map_or(0, |(at, _)| at)
        };
        let Some(decoded) = decode(&mut self.entries[at].pixels) else {
            self.entries.swap_remove(at);
            return None;
        };
        let entry = &mut self.entries[at];
        entry.key = key;
        entry.used = self.clock;
        entry.decoded = decoded;
        Some((entry.decoded, &entry.pixels))
    }
}

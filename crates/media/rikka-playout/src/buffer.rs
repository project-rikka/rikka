use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

const CAPACITY: usize = 100;
const TIMESTAMP_BASE: i64 = 1 << 32;

#[derive(Debug)]
pub(crate) struct Buffered {
    pub(crate) payload: Vec<u8>,
    pub(crate) frames: usize,
}

#[derive(Debug, Default)]
pub(crate) struct PacketBuffer {
    packets: BTreeMap<u64, Buffered>,
    newest: Option<u64>,
    frames: usize,
}

impl PacketBuffer {
    pub(crate) fn unwrap(&self, timestamp: u32) -> u64 {
        match self.newest {
            None => (TIMESTAMP_BASE + i64::from(timestamp)) as u64,
            Some(newest) => {
                let step = i64::from(timestamp.wrapping_sub(newest as u32) as i32);
                (newest as i64 + step).max(0) as u64
            }
        }
    }

    pub(crate) fn insert(&mut self, timestamp: u64, packet: Buffered) {
        if self.newest.is_none_or(|newest| timestamp > newest) {
            self.newest = Some(timestamp);
        }
        if let Entry::Vacant(vacant) = self.packets.entry(timestamp) {
            self.frames += packet.frames;
            vacant.insert(packet);
        }
        while self.packets.len() > CAPACITY {
            self.pop_first();
        }
    }

    pub(crate) fn first(&self) -> Option<(u64, &Buffered)> {
        self.packets
            .first_key_value()
            .map(|(at, packet)| (*at, packet))
    }

    pub(crate) fn take(&mut self, timestamp: u64) -> Option<Buffered> {
        let packet = self.packets.remove(&timestamp)?;
        self.frames -= packet.frames;
        Some(packet)
    }

    pub(crate) fn drop_before(&mut self, timestamp: u64) {
        while self
            .packets
            .first_key_value()
            .is_some_and(|(at, _)| *at < timestamp)
        {
            self.pop_first();
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.packets.is_empty()
    }

    pub(crate) fn frames(&self) -> usize {
        self.frames
    }

    fn pop_first(&mut self) {
        if let Some((_, packet)) = self.packets.pop_first() {
            self.frames -= packet.frames;
        }
    }
}

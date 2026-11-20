use std::collections::BTreeSet;

use crate::packetize::VideoFrame;

const REMEMBERED: usize = 1024;

#[derive(Debug, Default)]
pub struct Chain {
    stream: Option<u16>,
    good: BTreeSet<u32>,
}

impl Chain {
    #[must_use]
    pub fn new() -> Chain {
        Chain::default()
    }

    pub fn decodable(&mut self, frame: &VideoFrame) -> bool {
        if self.stream != Some(frame.stream) {
            self.stream = Some(frame.stream);
            self.good.clear();
        }
        frame.flags.intra || self.good.contains(&frame.reference)
    }

    pub fn decoded(&mut self, frame: &VideoFrame) {
        if self.stream != Some(frame.stream) {
            return;
        }
        if frame.flags.intra {
            self.good.retain(|known| *known > frame.frame);
        }
        self.good.insert(frame.frame);
        while self.good.len() > REMEMBERED {
            self.good.pop_first();
        }
    }

    pub fn retract(&mut self, stream: u16, frame: u32) {
        if self.stream == Some(stream) {
            self.good.retain(|known| *known < frame);
        }
    }

    #[must_use]
    pub fn newest_good(&self) -> Option<u32> {
        self.good.last().copied()
    }
}

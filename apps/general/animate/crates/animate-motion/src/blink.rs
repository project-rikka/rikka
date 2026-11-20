use crate::tuning::{
    BLINK_CLOSING_FRAMES, BLINK_GAP_SECONDS, BLINK_OPENING_FRAMES, DOUBLE_BLINK_CHANCE,
    DOUBLE_BLINK_FRAMES, FRAMES_PER_SECOND,
};

const MIXING: u64 = 0x9e37_79b9_7f4a_7c15;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Blinking {
    starts: Vec<u32>,
}

impl Blinking {
    pub(crate) fn new(seed: u64, until: u32) -> Self {
        let mut dice = Dice(seed ^ MIXING);
        let mut starts = Vec::new();
        let mut next = dice.gap();
        while next <= until {
            starts.push(next);
            if dice.uniform() < DOUBLE_BLINK_CHANCE {
                starts.push(next + DOUBLE_BLINK_FRAMES);
            }
            next = starts.last().copied().unwrap_or(next) + dice.gap();
        }
        Self { starts }
    }

    pub(crate) fn weight(&self, frame: f32) -> f32 {
        let length = BLINK_CLOSING_FRAMES + BLINK_OPENING_FRAMES;
        let first = self
            .starts
            .partition_point(|start| *start as f32 + length < frame);
        self.starts[first..]
            .iter()
            .take_while(|start| **start as f32 <= frame)
            .map(|start| shape(frame - *start as f32))
            .fold(0.0, f32::max)
    }
}

struct Dice(u64);

impl Dice {
    fn gap(&mut self) -> u32 {
        let (shortest, longest) = BLINK_GAP_SECONDS;
        let seconds = shortest + (longest - shortest) * self.uniform();
        (seconds * FRAMES_PER_SECOND).round() as u32
    }

    fn uniform(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(MIXING);
        let mut mixed = self.0;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        mixed ^= mixed >> 31;
        (mixed >> 40) as f32 / (1u64 << 24) as f32
    }
}

fn shape(since: f32) -> f32 {
    if since <= BLINK_CLOSING_FRAMES {
        since / BLINK_CLOSING_FRAMES
    } else {
        (1.0 - (since - BLINK_CLOSING_FRAMES) / BLINK_OPENING_FRAMES).max(0.0)
    }
}

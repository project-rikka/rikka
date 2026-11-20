use std::time::Instant;

use discord_core::{SAMPLE_RATE, VOICE_FRAME_SAMPLES};

use crate::rtp::{HEADER_LEN, Header};

#[derive(Debug)]
pub struct VoiceStream {
    ssrc: u32,
    sequence: u16,
    timestamp: u32,
    origin: Instant,
    base: u32,
    talkspurt: bool,
}

impl VoiceStream {
    #[must_use]
    pub fn new(ssrc: u32, origin: Instant) -> VoiceStream {
        let base = rand::random();
        VoiceStream {
            ssrc,
            sequence: rand::random(),
            timestamp: base,
            origin,
            base,
            talkspurt: true,
        }
    }

    pub fn talkspurt(&mut self) {
        self.talkspurt = true;
    }

    pub fn next(&mut self, now: Instant) -> [u8; HEADER_LEN] {
        let marker = std::mem::take(&mut self.talkspurt);
        if marker {
            let elapsed = now.saturating_duration_since(self.origin);
            let samples = elapsed.as_micros() * u128::from(SAMPLE_RATE) / 1_000_000;
            let wall = self.base.wrapping_add(samples as u32);
            if wall.wrapping_sub(self.timestamp).cast_signed() > 0 {
                self.timestamp = wall;
            }
        }
        let header = Header::opus(self.sequence, self.timestamp, self.ssrc, marker);
        self.sequence = self.sequence.wrapping_add(1);
        self.timestamp = self.timestamp.wrapping_add(VOICE_FRAME_SAMPLES as u32);
        header
    }
}

use rikka_apm_audio::{BAND, BANDS, Frame};

use crate::common::RENDER_TRANSFER_QUEUE_FRAMES;
use crate::framing::FrameBands;
use crate::high_pass::HighPassFilter;

#[derive(Clone, Debug)]
pub(crate) struct RenderQueue {
    high_pass: Option<HighPassFilter>,
    frames: Vec<Vec<FrameBands>>,
    head: usize,
    len: usize,
}

impl RenderQueue {
    pub(crate) fn new(channels: usize, high_pass_echo_reference: bool) -> RenderQueue {
        RenderQueue {
            high_pass: high_pass_echo_reference.then(|| HighPassFilter::new(channels)),
            frames: vec![vec![[[0.0; BAND]; BANDS]; channels]; RENDER_TRANSFER_QUEUE_FRAMES],
            head: 0,
            len: 0,
        }
    }

    pub(crate) fn insert(&mut self, frame: &Frame) {
        if self.len == self.frames.len() {
            return;
        }
        let slot = (self.head + self.len) % self.frames.len();
        let queued = &mut self.frames[slot];
        for (channel, bands) in queued.iter_mut().enumerate() {
            *bands = *frame.bands(channel);
        }
        if let Some(high_pass) = &mut self.high_pass {
            for (channel, bands) in queued.iter_mut().enumerate() {
                high_pass.process(channel, &mut bands[0]);
            }
        }
        self.len += 1;
    }

    pub(crate) fn remove(&mut self, out: &mut [FrameBands]) -> bool {
        if self.len == 0 {
            return false;
        }
        out.copy_from_slice(&self.frames[self.head]);
        self.head = (self.head + 1) % self.frames.len();
        self.len -= 1;
        true
    }
}

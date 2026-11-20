use std::time::Duration;

use crate::decoding::backend::{Backend, Block, Opened};
use crate::decoding::input::Input;
use crate::decoding::open;
use crate::decoding::window::{Cut, Window};
use crate::error::Result;
use crate::read::AudioInfo;

pub(crate) struct Pipeline {
    backend: Box<dyn Backend>,
    info: AudioInfo,
    window: Window,
    block: Block,
    ended: bool,
}

impl Pipeline {
    pub(crate) fn open(input: Input) -> Result<Pipeline> {
        let Opened { backend, info } = open::open(input)?;
        Ok(Pipeline {
            window: Window::new(info.frames, info.channels),
            backend,
            info,
            block: Block::default(),
            ended: false,
        })
    }

    pub(crate) fn info(&self) -> &AudioInfo {
        &self.info
    }

    pub(crate) fn next(&mut self, out: &mut Vec<f32>) -> Result<bool> {
        while !self.ended {
            self.block.samples.clear();
            if !self.backend.next(&mut self.block)? {
                self.ended = true;
                break;
            }
            match self.window.cut(&self.block, out) {
                Cut::Kept => return Ok(true),
                Cut::Skipped => {}
                Cut::Past => self.ended = true,
            }
        }
        Ok(false)
    }

    pub(crate) fn seek(&mut self, frame: u64) -> Result<u64> {
        let target = self.window.clamp(frame);
        self.backend.seek(target)?;
        self.window.start_at(target);
        self.ended = false;
        Ok(target)
    }

    pub(crate) fn seek_to(&mut self, at: Duration) -> Result<Duration> {
        let rate = f64::from(self.info.sample_rate.max(1));
        let frame = (at.as_secs_f64() * rate).round() as u64;
        let reached = self.seek(frame)?;
        Ok(Duration::from_secs_f64(reached as f64 / rate))
    }
}

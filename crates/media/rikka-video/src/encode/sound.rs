use std::path::Path;
use std::time::Duration;

use crate::audio::{AudioConfig, AudioTrack};
use crate::error::Result;
use rikka_ffmpeg::OutputContext;
use rikka_video_core::init;

pub struct AudioEncoder {
    track: Box<AudioTrack>,
    output: OutputContext,
    config: AudioConfig,
    frames: u64,
}

impl AudioEncoder {
    pub fn new(path: impl AsRef<Path>, config: AudioConfig) -> Result<Self> {
        init::ensure()?;
        let mut output = OutputContext::create(path.as_ref())?;
        let track = Box::new(AudioTrack::open(&output, config)?);
        let faststart = output.is_quicktime();
        output.write_header(faststart)?;
        Ok(Self {
            track,
            output,
            config,
            frames: 0,
        })
    }

    pub fn push(&mut self, samples: &[f32]) -> Result<()> {
        self.track.push(&mut self.output, samples)?;
        self.frames += (samples.len() / usize::from(self.config.channels().max(1))) as u64;
        Ok(())
    }

    pub fn finish(mut self) -> Result<Duration> {
        self.track.finish(&mut self.output)?;
        self.output.write_trailer()?;
        Ok(Duration::from_secs_f64(
            self.frames as f64 / f64::from(self.config.sample_rate().max(1)),
        ))
    }
}

impl std::fmt::Debug for AudioEncoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioEncoder")
            .field("config", &self.config)
            .field("frames", &self.frames)
            .finish_non_exhaustive()
    }
}

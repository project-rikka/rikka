use crate::audio::{AudioConfig, SoundCoder};
use crate::coded::packet::from_packet;
use crate::coded::stream::StreamInfo;
use crate::error::Result;
use rikka_coded::Coded;
use rikka_video_core::init;

pub struct SoundPackets {
    coder: SoundCoder,
}

impl SoundPackets {
    pub fn open(config: AudioConfig) -> Result<SoundPackets> {
        init::ensure()?;
        Ok(SoundPackets {
            coder: SoundCoder::open(config, true)?,
        })
    }

    pub fn stream(&self) -> Result<StreamInfo> {
        StreamInfo::from_context(self.coder.context(), self.coder.time_base(), None)
    }

    pub fn push(&mut self, samples: &[f32]) -> Result<Vec<Coded>> {
        let time_base = self.coder.time_base();
        let mut coded = Vec::new();
        self.coder.push(samples, &mut |packet| {
            coded.extend(from_packet(packet, time_base));
            Ok(())
        })?;
        Ok(coded)
    }

    pub fn finish(mut self) -> Result<Vec<Coded>> {
        let time_base = self.coder.time_base();
        let mut coded = Vec::new();
        self.coder.finish(&mut |packet| {
            coded.extend(from_packet(packet, time_base));
            Ok(())
        })?;
        Ok(coded)
    }
}

impl std::fmt::Debug for SoundPackets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SoundPackets")
            .field("coder", &self.coder)
            .finish()
    }
}

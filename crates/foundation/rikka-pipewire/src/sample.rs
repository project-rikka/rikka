use libspa::param::audio::{AudioFormat, AudioInfoRaw};
use libspa::pod::Pod;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sample {
    S16,
    S24,
    S24In32,
    S32,
    F32,
    F64,
}

impl Sample {
    #[must_use]
    pub fn from_audio_format(format: AudioFormat) -> Option<Sample> {
        match format {
            AudioFormat::S16LE | AudioFormat::S16BE | AudioFormat::S16P => Some(Sample::S16),
            AudioFormat::S24LE | AudioFormat::S24BE | AudioFormat::S24P => Some(Sample::S24),
            AudioFormat::S24_32LE | AudioFormat::S24_32BE | AudioFormat::S24_32P => {
                Some(Sample::S24In32)
            }
            AudioFormat::S32LE | AudioFormat::S32BE | AudioFormat::S32P => Some(Sample::S32),
            AudioFormat::F32LE | AudioFormat::F32BE | AudioFormat::F32P => Some(Sample::F32),
            AudioFormat::F64LE | AudioFormat::F64BE | AudioFormat::F64P => Some(Sample::F64),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RawFormat {
    pub sample: Option<Sample>,
    pub rate: u32,
    pub channels: u32,
}

impl RawFormat {
    #[must_use]
    pub fn parse(param: &Pod) -> Option<RawFormat> {
        let mut info = AudioInfoRaw::new();
        info.parse(param).ok()?;
        Some(RawFormat {
            sample: Sample::from_audio_format(info.format()),
            rate: info.rate(),
            channels: info.channels(),
        })
    }
}

use rikka_audio_decode::WavDepth;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SoundCodec {
    #[default]
    Wav,
    Aac,
}

impl SoundCodec {
    pub const ALL: [SoundCodec; 2] = [SoundCodec::Wav, SoundCodec::Aac];

    #[must_use]
    pub fn extension(self) -> &'static str {
        match self {
            SoundCodec::Wav => "wav",
            SoundCodec::Aac => "m4a",
        }
    }

    #[must_use]
    pub fn mime_type(self) -> &'static str {
        match self {
            SoundCodec::Wav => "audio/wav",
            SoundCodec::Aac => "audio/mp4",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Audio {
    pub codec: SoundCodec,
    pub depth: WavDepth,
    pub sample_rate: u32,
    pub bitrate: u32,
}

pub const AAC_BITRATES: [u32; 4] = [128, 192, 256, 320];

impl Audio {
    pub const DEPTHS: [WavDepth; 2] = [WavDepth::Pcm16, WavDepth::Pcm24];
    pub const SAMPLE_RATES: [u32; 3] = [44_100, 48_000, 96_000];
}

impl Default for Audio {
    fn default() -> Audio {
        Audio {
            codec: SoundCodec::Wav,
            depth: WavDepth::Pcm24,
            sample_rate: 48_000,
            bitrate: 256,
        }
    }
}

use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Codec {
    Pcm,
    FloatPcm,
    Flac,
    Alac,
    WavPack,
    Ape,
    Tak,
    Tta,
    Dsd,
    Mp1,
    Mp3,
    Mp2,
    Aac,
    HeAac,
    HeAacV2,
    Vorbis,
    Opus,
    Speex,
    Musepack,
    Wma,
    WmaLossless,
    Adpcm,
    Ac3,
    Eac3,
    Other(&'static str),
}

impl Codec {
    #[must_use]
    pub fn is_lossless(self) -> bool {
        matches!(
            self,
            Codec::Pcm
                | Codec::FloatPcm
                | Codec::Flac
                | Codec::Alac
                | Codec::WavPack
                | Codec::Ape
                | Codec::Tak
                | Codec::Tta
                | Codec::Dsd
                | Codec::WmaLossless
        )
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Codec::Pcm => "PCM",
            Codec::FloatPcm => "PCM float",
            Codec::Flac => "FLAC",
            Codec::Alac => "ALAC",
            Codec::WavPack => "WavPack",
            Codec::Ape => "Monkey's Audio",
            Codec::Tak => "TAK",
            Codec::Tta => "TTA",
            Codec::Dsd => "DSD",
            Codec::Mp1 => "MP1",
            Codec::Mp3 => "MP3",
            Codec::Mp2 => "MP2",
            Codec::Aac => "AAC",
            Codec::HeAac => "HE-AAC",
            Codec::HeAacV2 => "HE-AAC v2",
            Codec::Vorbis => "Vorbis",
            Codec::Opus => "Opus",
            Codec::Speex => "Speex",
            Codec::Musepack => "Musepack",
            Codec::Wma => "WMA",
            Codec::WmaLossless => "WMA Lossless",
            Codec::Adpcm => "ADPCM",
            Codec::Ac3 => "AC-3",
            Codec::Eac3 => "E-AC-3",
            Codec::Other(name) => name,
        }
    }
}

impl fmt::Display for Codec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Encoding {
    pub codec: Codec,
    pub lossless: bool,
    pub bitrate: Option<u32>,
    pub dsd_rate: Option<u32>,
}

impl Encoding {
    pub(crate) fn for_codec(codec: Codec) -> Encoding {
        Encoding {
            codec,
            lossless: codec.is_lossless(),
            bitrate: None,
            dsd_rate: None,
        }
    }
}

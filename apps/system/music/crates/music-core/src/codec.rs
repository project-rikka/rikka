use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Codec {
    Flac,
    Alac,
    Pcm,
    FloatPcm,
    Ape,
    WavPack,
    Tak,
    Tta,
    Dsd,
    Mp1,
    Mp2,
    Mp3,
    Aac,
    Opus,
    Vorbis,
    Speex,
    Musepack,
    Wma,
    Ac3,
    Eac3,
    Other,
}

const CODECS: [(Codec, &str, &str); 21] = [
    (Codec::Flac, "flac", "FLAC"),
    (Codec::Alac, "alac", "ALAC"),
    (Codec::Pcm, "pcm", "PCM"),
    (Codec::FloatPcm, "float-pcm", "PCM float"),
    (Codec::Ape, "ape", "Monkey's Audio"),
    (Codec::WavPack, "wavpack", "WavPack"),
    (Codec::Tak, "tak", "TAK"),
    (Codec::Tta, "tta", "TTA"),
    (Codec::Dsd, "dsd", "DSD"),
    (Codec::Mp1, "mp1", "MP1"),
    (Codec::Mp2, "mp2", "MP2"),
    (Codec::Mp3, "mp3", "MP3"),
    (Codec::Aac, "aac", "AAC"),
    (Codec::Opus, "opus", "Opus"),
    (Codec::Vorbis, "vorbis", "Vorbis"),
    (Codec::Speex, "speex", "Speex"),
    (Codec::Musepack, "musepack", "Musepack"),
    (Codec::Wma, "wma", "WMA"),
    (Codec::Ac3, "ac3", "AC-3"),
    (Codec::Eac3, "eac3", "E-AC-3"),
    (Codec::Other, "other", "Unknown"),
];

impl Codec {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        CODECS
            .iter()
            .find(|(codec, _, _)| *codec == self)
            .map_or("other", |(_, slug, _)| slug)
    }

    #[must_use]
    pub fn from_slug(slug: &str) -> Codec {
        CODECS
            .iter()
            .find(|(_, known, _)| *known == slug)
            .map_or(Codec::Other, |(codec, _, _)| *codec)
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        CODECS
            .iter()
            .find(|(codec, _, _)| *codec == self)
            .map_or("Unknown", |(_, _, label)| label)
    }

    #[must_use]
    pub fn is_lossless(self) -> bool {
        matches!(
            self,
            Codec::Flac
                | Codec::Alac
                | Codec::Pcm
                | Codec::FloatPcm
                | Codec::Ape
                | Codec::WavPack
                | Codec::Tak
                | Codec::Tta
                | Codec::Dsd
        )
    }
}

impl From<rikka_audio_decode::Codec> for Codec {
    fn from(codec: rikka_audio_decode::Codec) -> Codec {
        match codec {
            rikka_audio_decode::Codec::Pcm => Codec::Pcm,
            rikka_audio_decode::Codec::FloatPcm => Codec::FloatPcm,
            rikka_audio_decode::Codec::Flac => Codec::Flac,
            rikka_audio_decode::Codec::Alac => Codec::Alac,
            rikka_audio_decode::Codec::WavPack => Codec::WavPack,
            rikka_audio_decode::Codec::Ape => Codec::Ape,
            rikka_audio_decode::Codec::Tak => Codec::Tak,
            rikka_audio_decode::Codec::Tta => Codec::Tta,
            rikka_audio_decode::Codec::Dsd => Codec::Dsd,
            rikka_audio_decode::Codec::Mp1 => Codec::Mp1,
            rikka_audio_decode::Codec::Mp2 => Codec::Mp2,
            rikka_audio_decode::Codec::Mp3 => Codec::Mp3,
            rikka_audio_decode::Codec::Aac
            | rikka_audio_decode::Codec::HeAac
            | rikka_audio_decode::Codec::HeAacV2 => Codec::Aac,
            rikka_audio_decode::Codec::Vorbis => Codec::Vorbis,
            rikka_audio_decode::Codec::Opus => Codec::Opus,
            rikka_audio_decode::Codec::Speex => Codec::Speex,
            rikka_audio_decode::Codec::Musepack => Codec::Musepack,
            rikka_audio_decode::Codec::Wma | rikka_audio_decode::Codec::WmaLossless => Codec::Wma,
            rikka_audio_decode::Codec::Ac3 => Codec::Ac3,
            rikka_audio_decode::Codec::Eac3 => Codec::Eac3,
            rikka_audio_decode::Codec::Adpcm | rikka_audio_decode::Codec::Other(_) => Codec::Other,
        }
    }
}

impl fmt::Display for Codec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

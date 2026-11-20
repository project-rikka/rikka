#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioFormat {
    Wav,
    Aiff,
    Caf,
    Flac,
    Mp3,
    Aac,
    Ogg,
    Mp4,
    Mkv,
    Opus,
    WavPack,
    Ape,
    Dsf,
    Dff,
    Tak,
    Musepack,
    Wma,
    Ac3,
    Eac3,
    Tta,
}

impl AudioFormat {
    pub fn from_extension(extension: &str) -> Option<AudioFormat> {
        let format = match extension.to_ascii_lowercase().as_str() {
            "wav" | "wave" => AudioFormat::Wav,
            "aiff" | "aif" | "aifc" => AudioFormat::Aiff,
            "caf" => AudioFormat::Caf,
            "flac" => AudioFormat::Flac,
            "mp3" | "mp2" | "mp1" => AudioFormat::Mp3,
            "aac" | "adts" => AudioFormat::Aac,
            "ogg" | "oga" => AudioFormat::Ogg,
            "mp4" | "m4a" | "m4b" => AudioFormat::Mp4,
            "mkv" | "mka" | "webm" => AudioFormat::Mkv,
            "opus" => AudioFormat::Opus,
            "wv" => AudioFormat::WavPack,
            "ape" => AudioFormat::Ape,
            "dsf" => AudioFormat::Dsf,
            "dff" => AudioFormat::Dff,
            "tak" => AudioFormat::Tak,
            "mpc" | "mp+" | "mpp" => AudioFormat::Musepack,
            "wma" | "asf" => AudioFormat::Wma,
            "ac3" => AudioFormat::Ac3,
            "eac3" | "ec3" => AudioFormat::Eac3,
            "tta" => AudioFormat::Tta,
            _ => return None,
        };
        Some(format)
    }

    pub(crate) fn extension(self) -> &'static str {
        match self {
            AudioFormat::Wav => "wav",
            AudioFormat::Aiff => "aiff",
            AudioFormat::Caf => "caf",
            AudioFormat::Flac => "flac",
            AudioFormat::Mp3 => "mp3",
            AudioFormat::Aac => "aac",
            AudioFormat::Ogg | AudioFormat::Opus => "ogg",
            AudioFormat::Mp4 => "m4a",
            AudioFormat::Mkv => "mka",
            AudioFormat::WavPack => "wv",
            AudioFormat::Ape => "ape",
            AudioFormat::Dsf => "dsf",
            AudioFormat::Dff => "dff",
            AudioFormat::Tak => "tak",
            AudioFormat::Musepack => "mpc",
            AudioFormat::Wma => "wma",
            AudioFormat::Ac3 => "ac3",
            AudioFormat::Eac3 => "eac3",
            AudioFormat::Tta => "tta",
        }
    }
}

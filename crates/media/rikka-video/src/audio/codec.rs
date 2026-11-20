#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AudioCodec {
    #[default]
    Aac,
    Opus,
    Mp3,
    Vorbis,
    Flac,
    Alac,
}

impl AudioCodec {
    #[must_use]
    pub const fn is_lossless(self) -> bool {
        matches!(self, AudioCodec::Flac | AudioCodec::Alac)
    }

    pub(crate) const fn encoders(self) -> &'static [&'static str] {
        match self {
            AudioCodec::Aac => &["libfdk_aac", "aac"],
            AudioCodec::Opus => &["libopus"],
            AudioCodec::Mp3 => &["libmp3lame"],
            AudioCodec::Vorbis => &["libvorbis"],
            AudioCodec::Flac => &["flac"],
            AudioCodec::Alac => &["alac"],
        }
    }

    pub(crate) const fn stream_name(self) -> &'static str {
        match self {
            AudioCodec::Aac => "aac",
            AudioCodec::Opus => "opus",
            AudioCodec::Mp3 => "mp3",
            AudioCodec::Vorbis => "vorbis",
            AudioCodec::Flac => "flac",
            AudioCodec::Alac => "alac",
        }
    }

    pub(crate) const fn bitrate_per_channel(self, quality: u8) -> u64 {
        let quality = if quality > 100 { 100 } else { quality } as u64;
        match self {
            AudioCodec::Opus => 16_000 + quality * 80_000 / 100,
            _ => 32_000 + quality * 96_000 / 100,
        }
    }
}

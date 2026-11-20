use crate::encoding::Codec;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Ffmpeg(#[from] rikka_ffmpeg::Error),

    #[error("no decodable audio track")]
    NoAudioTrack,

    #[error("decoding {0} requires FFmpeg, which is not installed")]
    NeedsFfmpeg(Codec),

    #[error("FFmpeg has no {0} decoder")]
    NoDecoder(Codec),

    #[error("FFmpeg sample format {0} is not supported")]
    SampleFormat(i32),

    #[error("no uncompressed DSD stream")]
    NotDsd,

    #[error("a WAV file cannot hold {0} channels")]
    WavChannels(u16),

    #[error("{samples} samples do not make whole frames of {channels} channels")]
    PartialFrame { samples: usize, channels: u16 },

    #[error("WAV data exceeds 4 GiB")]
    WavTooLarge,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

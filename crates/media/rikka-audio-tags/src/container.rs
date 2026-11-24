use std::io::{Read, Seek};

use lofty::aac::AacFile;
use lofty::ape::ApeFile;
use lofty::config::{ParseOptions, ParsingMode};
use lofty::file::{AudioFile, FileType, TaggedFile, TaggedFileExt};
use lofty::flac::FlacFile;
use lofty::iff::aiff::{AiffCompressionType, AiffFile, AiffProperties};
use lofty::iff::wav::{WavFile, WavFormat, WavProperties};
use lofty::mp4::{Mp4Codec, Mp4File, Mp4Properties};
use lofty::mpeg::{Layer, MpegFile, MpegProperties};
use lofty::musepack::MpcFile;
use lofty::ogg::{OpusFile, SpeexFile, VorbisFile};
use lofty::properties::FileProperties;
use lofty::tag::{Tag, TagType};
use lofty::wavpack::{WavPackFile, WavPackProperties};

use crate::audio::Audio;
use crate::error::{Error, Result};
use rikka_audio_decode::Codec;

const WAVE_MP3: u16 = 0x0055;
const PREFERRED_TAGS: [TagType; 7] = [
    TagType::Id3v2,
    TagType::VorbisComments,
    TagType::Mp4Ilst,
    TagType::Ape,
    TagType::RiffInfo,
    TagType::AiffText,
    TagType::Id3v1,
];

pub(crate) struct Opened {
    pub(crate) audio: Audio,
    pub(crate) tags: Vec<Tag>,
}

pub(crate) fn options() -> ParseOptions {
    ParseOptions::new()
        .parsing_mode(ParsingMode::BestAttempt)
        .read_cover_art(true)
}

pub(crate) fn read<R: Read + Seek>(reader: &mut R, file_type: FileType) -> Result<Opened> {
    match file_type {
        FileType::Flac => open::<FlacFile, _>(reader, |_| (Codec::Flac, true)),
        FileType::Mpeg => open::<MpegFile, _>(reader, |properties: &MpegProperties| {
            let codec = match properties.layer() {
                Layer::Layer1 => Codec::Mp1,
                Layer::Layer2 => Codec::Mp2,
                Layer::Layer3 => Codec::Mp3,
            };
            (codec, false)
        }),
        FileType::Mp4 => open::<Mp4File, _>(reader, |properties: &Mp4Properties| match properties
            .codec()
        {
            Some(Mp4Codec::ALAC) => (Codec::Alac, true),
            Some(Mp4Codec::FLAC) => (Codec::Flac, true),
            Some(Mp4Codec::MP3) => (Codec::Mp3, false),
            Some(Mp4Codec::AAC) => (Codec::Aac, false),
            _ => (Codec::Other("MP4"), false),
        }),
        FileType::Wav => open::<WavFile, _>(reader, |properties: &WavProperties| match properties
            .format()
        {
            WavFormat::PCM => (Codec::Pcm, true),
            WavFormat::IEEE_FLOAT => (Codec::FloatPcm, true),
            WavFormat::Other(WAVE_MP3) => (Codec::Mp3, false),
            WavFormat::Other(_) => (Codec::Other("WAV"), false),
        }),
        FileType::Aiff => {
            open::<AiffFile, _>(reader, |properties: &AiffProperties| {
                match properties.compression_type() {
                    None | Some(AiffCompressionType::None | AiffCompressionType::sowt) => {
                        (Codec::Pcm, true)
                    }
                    Some(
                        AiffCompressionType::fl32
                        | AiffCompressionType::fl64
                        | AiffCompressionType::FL32,
                    ) => (Codec::FloatPcm, true),
                    Some(_) => (Codec::Other("AIFF"), false),
                }
            })
        }
        FileType::Ape => open::<ApeFile, _>(reader, |_| (Codec::Ape, true)),
        FileType::WavPack => open::<WavPackFile, _>(reader, |properties: &WavPackProperties| {
            (Codec::WavPack, properties.is_lossless())
        }),
        FileType::Opus => open::<OpusFile, _>(reader, |_| (Codec::Opus, false)),
        FileType::Vorbis => open::<VorbisFile, _>(reader, |_| (Codec::Vorbis, false)),
        FileType::Speex => open::<SpeexFile, _>(reader, |_| (Codec::Speex, false)),
        FileType::Mpc => open::<MpcFile, _>(reader, |_| (Codec::Musepack, false)),
        FileType::Aac => open::<AacFile, _>(reader, |_| (Codec::Aac, false)),
        _ => Err(Error::Unknown),
    }
}

fn open<F, R>(reader: &mut R, codec: impl FnOnce(&F::Properties) -> (Codec, bool)) -> Result<Opened>
where
    F: AudioFile,
    F::Properties: Clone + Into<FileProperties>,
    R: Read + Seek,
{
    let file = F::read_from(reader, options())?;
    let (codec, lossless) = codec(file.properties());
    let properties: FileProperties = file.properties().clone().into();
    let mut tagged: TaggedFile = file.into();
    Ok(Opened {
        audio: Audio {
            codec,
            lossless,
            duration: properties.duration(),
            sample_rate: properties.sample_rate().filter(|rate| *rate > 0),
            bit_depth: properties.bit_depth().filter(|bits| *bits > 0),
            channels: properties.channels().filter(|channels| *channels > 0),
            bitrate: properties
                .audio_bitrate()
                .or(properties.overall_bitrate())
                .filter(|bitrate| *bitrate > 0),
        },
        tags: taken(&mut tagged),
    })
}

pub(crate) fn taken(tagged: &mut TaggedFile) -> Vec<Tag> {
    let primary = tagged.primary_tag_type();
    std::iter::once(primary)
        .chain(PREFERRED_TAGS.into_iter().filter(|kind| *kind != primary))
        .filter_map(|kind| tagged.remove(kind))
        .collect()
}

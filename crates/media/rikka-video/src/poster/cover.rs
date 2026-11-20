use crate::error::{Error, Result};
use rikka_ffmpeg::Frame;
use rikka_ffmpeg::avcodec;
use rikka_ffmpeg::avformat::{self, AVStream};
use rikka_ffmpeg::avutil;
use rikka_video_core::decode::{Decoded, Decoder};
use rikka_video_core::demux::Demuxer;

pub(super) fn largest(demuxer: &Demuxer) -> Option<&AVStream> {
    (0..demuxer.stream_count())
        .map(|index| demuxer.stream_at(index))
        .filter(|stream| is_cover(stream))
        .max_by_key(|stream| {
            let parameters = unsafe { &*stream.codecpar };
            i64::from(parameters.width.max(0)) * i64::from(parameters.height.max(0))
        })
}

fn is_cover(stream: &AVStream) -> bool {
    let parameters = unsafe { &*stream.codecpar };
    stream.disposition & avformat::AV_DISPOSITION_ATTACHED_PIC != 0
        && parameters.codec_type == avutil::AVMEDIA_TYPE_VIDEO
        && stream.attached_pic.size > 0
        && !stream.attached_pic.data.is_null()
}

pub(super) fn decode(stream: &AVStream, into: &mut Frame) -> Result<()> {
    let parameters = stream.codecpar;
    let codec = unsafe { avcodec::avcodec_find_decoder((*parameters).codec_id) };
    if codec.is_null() {
        return Err(Error::Unsupported(
            "no decoder for the attached picture".into(),
        ));
    }
    let mut decoder = Decoder::for_stills(codec, parameters, stream.time_base)?;
    decoder.send_borrowed(&stream.attached_pic)?;
    decoder.send(None)?;
    match decoder.receive(into)? {
        Decoded::Frame => Ok(()),
        Decoded::NeedsInput | Decoded::Eof => {
            Err(Error::Unsupported("attached picture is empty".into()))
        }
    }
}

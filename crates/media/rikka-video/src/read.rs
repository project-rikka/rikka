use std::path::Path;

use rikka_image::Image;
pub use rikka_video_core::VideoInfo;

use crate::error::{Error, Result};
use rikka_ffmpeg::Frame;
use rikka_ffmpeg::Packet;
use rikka_video_core::decode::{Decoded, Decoder};
use rikka_video_core::demux::Demuxer;
use rikka_video_core::init;
use rikka_video_core::sw::SwConverter;

pub fn probe(path: impl AsRef<Path>) -> Result<VideoInfo> {
    Ok(rikka_video_core::probe(path)?)
}

pub fn first_frame(path: impl AsRef<Path>) -> Result<Image> {
    let path = path.as_ref();
    init::ensure()?;

    let mut demuxer = Demuxer::open(path)?;
    let mut decoder = Decoder::open(
        demuxer.decoder(),
        demuxer.stream().codecpar,
        demuxer.time_base(),
        None,
    )?;

    let mut packet = Packet::new()?;
    let mut frame = Frame::new()?;
    let mut converter = SwConverter::new();

    loop {
        let read = demuxer.read(&mut packet)?;
        decoder.send(read.then_some(&mut packet))?;
        packet.unref();

        match decoder.receive(&mut frame)? {
            Decoded::Frame => return picture(&mut converter, &frame),
            Decoded::NeedsInput => {}
            Decoded::Eof => {
                return Err(Error::Unsupported(format!(
                    "{} has no frame to show",
                    path.display()
                )));
            }
        }
    }
}

fn picture(converter: &mut SwConverter, frame: &Frame) -> Result<Image> {
    let mut rgba = Vec::new();
    converter.rgba(frame, &mut rgba)?;
    Ok(Image::from_rgba8(
        frame.width().max(0) as u32,
        frame.height().max(0) as u32,
        rgba,
    )?)
}

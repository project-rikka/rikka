use std::path::Path;
use std::ptr;

use rikka_ffmpeg::avcodec;
use rikka_ffmpeg::avformat;
use rikka_ffmpeg::{OutputContext, Packet};

use crate::coded::packet::MICROS;
use crate::coded::stream::StreamInfo;
use crate::error::{Error, Result, check};
use rikka_coded::Sample;
use rikka_video_core::init;

pub struct Muxer {
    output: OutputContext,
    streams: usize,
    packet: Packet,
}

impl Muxer {
    pub fn create(path: impl AsRef<Path>, streams: &[&StreamInfo]) -> Result<Muxer> {
        init::ensure()?;
        if streams.is_empty() {
            return Err(Error::NothingToWrite);
        }
        let mut output = OutputContext::create(path.as_ref())?;
        for info in streams {
            add_stream(&output, info)?;
        }
        output.write_header(true)?;
        Ok(Muxer {
            output,
            streams: streams.len(),
            packet: Packet::new()?,
        })
    }

    pub fn write(&mut self, stream: usize, sample: Sample<'_>) -> Result<()> {
        if stream >= self.streams {
            return Err(Error::Unsupported(format!("no stream {stream} in muxer")));
        }
        write_sample(&mut self.output, &mut self.packet, stream, sample)
    }

    pub fn finish(mut self) -> Result<()> {
        Ok(self.output.write_trailer()?)
    }
}

pub(crate) fn write_sample(
    output: &mut OutputContext,
    packet: &mut Packet,
    stream: usize,
    sample: Sample<'_>,
) -> Result<()> {
    let target = unsafe { (*(*(*output.as_ptr()).streams.add(stream))).time_base };
    packet.unref();
    let written = packet.with_borrowed_data(sample.data, |packet| {
        packet.place(
            sample.pts_us,
            sample.dts_us,
            sample.duration_us.max(0),
            sample.keyframe,
        );
        packet.set_stream_index(stream as i32);
        packet.rescale(MICROS, target);
        output.write(packet, "av_interleaved_write_frame")
    });
    packet.unref();
    Ok(written?)
}

pub(crate) fn add_stream(output: &OutputContext, info: &StreamInfo) -> Result<()> {
    let stream = unsafe { avformat::avformat_new_stream(output.as_ptr(), ptr::null()) };
    if stream.is_null() {
        return Err(Error::Unsupported("avformat_new_stream failed".into()));
    }
    unsafe {
        let copied = avcodec::avcodec_parameters_copy((*stream).codecpar, info.params());
        check("avcodec_parameters_copy", copied)?;
        (*(*stream).codecpar).codec_tag = match info.is_hevc() && output.is_quicktime() {
            true => u32::from_le_bytes(*b"hvc1"),
            false => 0,
        };
        (*stream).time_base = info.time_base();
        if let Some(rate) = info.frame_rate() {
            (*stream).avg_frame_rate = rate;
        }
    }
    Ok(())
}

impl std::fmt::Debug for Muxer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Muxer")
            .field("streams", &self.streams)
            .finish_non_exhaustive()
    }
}

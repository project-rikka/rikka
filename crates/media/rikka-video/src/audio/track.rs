use std::ptr;

use rikka_ffmpeg::avcodec;
use rikka_ffmpeg::avformat;
use rikka_ffmpeg::avutil::AVRational;
use rikka_ffmpeg::{OutputContext, Packet};

use crate::audio::AudioConfig;
use crate::audio::coder::SoundCoder;
use crate::error::{Error, Result, check};

pub(crate) struct AudioTrack {
    coder: SoundCoder,
    stream_index: i32,
}

impl AudioTrack {
    pub(crate) fn open(output: &OutputContext, config: AudioConfig) -> Result<Self> {
        let coder = SoundCoder::open(config, output.wants_global_header())?;
        let stream = unsafe { avformat::avformat_new_stream(output.as_ptr(), ptr::null()) };
        if stream.is_null() {
            return Err(Error::Unsupported(
                "avformat_new_stream failed for audio".into(),
            ));
        }
        let stream_index = unsafe {
            (*stream).time_base = coder.time_base();
            let copied =
                avcodec::avcodec_parameters_from_context((*stream).codecpar, coder.context());
            check("avcodec_parameters_from_context(audio)", copied)?;
            (*stream).index
        };
        Ok(Self {
            coder,
            stream_index,
        })
    }

    pub(crate) fn push(&mut self, output: &mut OutputContext, samples: &[f32]) -> Result<()> {
        let (index, time_base) = (self.stream_index, self.coder.time_base());
        self.coder.push(samples, &mut |packet| {
            write(output, packet, index, time_base)
        })
    }

    pub(crate) fn finish(&mut self, output: &mut OutputContext) -> Result<()> {
        let (index, time_base) = (self.stream_index, self.coder.time_base());
        self.coder
            .finish(&mut |packet| write(output, packet, index, time_base))
    }
}

fn write(
    output: &mut OutputContext,
    packet: &mut Packet,
    index: i32,
    time_base: AVRational,
) -> Result<()> {
    let target = unsafe { (*(*(*output.as_ptr()).streams.add(index as usize))).time_base };
    packet.set_stream_index(index);
    packet.rescale(time_base, target);
    Ok(output.write(packet, "av_interleaved_write_frame(audio)")?)
}

impl std::fmt::Debug for AudioTrack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioTrack")
            .field("coder", &self.coder)
            .field("stream", &self.stream_index)
            .finish()
    }
}

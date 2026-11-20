use std::ptr;

use rikka_ffmpeg::avcodec;
use rikka_ffmpeg::avformat::AVStream;
use rikka_ffmpeg::avutil::{self, AVRational};
use rikka_ffmpeg::{CodecContext, Frame, OutputContext, Packet};

use crate::audio::{AudioConfig, AudioTrack, append_decoded};
use crate::convert::carry::shift;
use crate::convert::target::AudioTarget;
use crate::error::{Error, Result, check};

pub(crate) struct AudioLane {
    decoder: CodecContext,
    frame: Frame,
    track: AudioTrack,
    samples: Vec<f32>,
    channels: usize,
    rate: i32,
    time_base: AVRational,
    shift: i64,
    placed: bool,
}

impl AudioLane {
    pub(crate) fn open(
        stream: &AVStream,
        output: &OutputContext,
        target: &AudioTarget,
        start_us: i64,
    ) -> Result<AudioLane> {
        let parameters = unsafe { &*stream.codecpar };
        let codec = unsafe { avcodec::avcodec_find_decoder(parameters.codec_id) };
        if codec.is_null() {
            return Err(Error::Unsupported(
                "FFmpeg has no decoder for the audio stream".into(),
            ));
        }
        let decoder = unsafe { CodecContext::alloc(codec) }?;
        unsafe {
            let applied = avcodec::avcodec_parameters_to_context(decoder.as_ptr(), stream.codecpar);
            check("avcodec_parameters_to_context(audio)", applied)?;
            (*decoder.as_ptr()).pkt_timebase = stream.time_base;
            let status = avcodec::avcodec_open2(decoder.as_ptr(), codec, ptr::null_mut());
            check("avcodec_open2(audio decoder)", status)?;
        }

        let channels = parameters.ch_layout.nb_channels.max(1);
        let rate = parameters.sample_rate.max(1);
        let depth = match parameters.bits_per_raw_sample {
            bits if bits > 0 => bits,
            _ => parameters.bits_per_coded_sample.max(16),
        };
        let config = AudioConfig::new(rate as u32, channels as u16)
            .codec(target.codec)
            .rate(target.rate)
            .speed(target.speed)
            .bits_per_sample(depth as u32);
        let track = AudioTrack::open(output, config)?;
        Ok(AudioLane {
            decoder,
            frame: Frame::new()?,
            track,
            samples: Vec::new(),
            channels: channels as usize,
            rate,
            time_base: stream.time_base,
            shift: shift(start_us, stream.time_base),
            placed: false,
        })
    }

    pub(crate) fn feed(
        &mut self,
        packet: Option<&mut Packet>,
        output: &mut OutputContext,
    ) -> Result<()> {
        let ending = packet.is_none();
        let raw = packet.map_or(ptr::null(), |packet| packet.as_mut_ptr().cast_const());
        let sent = unsafe { avcodec::avcodec_send_packet(self.decoder.as_ptr(), raw) };
        if sent < 0 && sent != avutil::AVERROR_EOF {
            if ending {
                check("avcodec_send_packet(audio)", sent)?;
            }
            tracing::debug!("skipping corrupt audio packet (AVERROR {sent})");
            return Ok(());
        }
        loop {
            let received = unsafe {
                avcodec::avcodec_receive_frame(self.decoder.as_ptr(), self.frame.as_mut_ptr())
            };
            if received == avutil::AVERROR_EAGAIN {
                return Ok(());
            }
            if received == avutil::AVERROR_EOF {
                return self.track.finish(output);
            }
            if received < 0 {
                tracing::debug!("failed to decode audio frame (AVERROR {received})");
                continue;
            }
            self.take(output)?;
        }
    }

    fn take(&mut self, output: &mut OutputContext) -> Result<()> {
        if self.frame.channels() != self.channels {
            self.frame.unref();
            return Err(Error::Unsupported(
                "channel count changes mid-stream".into(),
            ));
        }
        self.samples.clear();
        let lead = match self.placed {
            true => 0,
            false => {
                self.placed = true;
                self.lead()
            }
        };
        if lead > 0 {
            self.samples.resize(lead as usize * self.channels, 0.0);
        }
        append_decoded(&self.frame, &mut self.samples);
        self.frame.unref();
        if lead < 0 {
            let early = (lead.unsigned_abs() as usize * self.channels).min(self.samples.len());
            self.samples.drain(..early);
        }
        self.track.push(output, &self.samples)
    }

    fn lead(&self) -> i64 {
        let Some(stamp) = self.frame.timestamp() else {
            return 0;
        };
        let frames = unsafe {
            avutil::av_rescale_q(
                stamp - self.shift,
                self.time_base,
                AVRational {
                    num: 1,
                    den: self.rate,
                },
            )
        };
        frames.clamp(-i64::from(self.rate) * 60, i64::from(self.rate) * 60)
    }
}

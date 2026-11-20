mod carry;
mod sound;
mod source;
mod target;
mod video;

use std::path::{Path, PathBuf};
use std::time::Duration;

use rikka_ffmpeg::avutil;
use rikka_ffmpeg::{OutputContext, Packet};
use rikka_types::Flow;

use crate::error::{Error, Result, check};
use crate::same_file::same_file;
use rikka_video_core::init;

use carry::CopiedStream;
use sound::AudioLane;
use source::{Described, Source};
use video::VideoLane;

pub use target::{AudioTarget, VideoTarget};

pub fn convert(source: impl AsRef<Path>, destination: impl AsRef<Path>) -> Converting {
    Converting {
        source: source.as_ref().to_path_buf(),
        destination: destination.as_ref().to_path_buf(),
        video: None,
        audio: None,
        subtitles: false,
        progress: None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handling {
    Copied,
    Encoded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Converted {
    pub video: Option<Handling>,
    pub audio: Option<Handling>,
    pub duration: Option<Duration>,
}

#[must_use]
pub struct Converting {
    source: PathBuf,
    destination: PathBuf,
    video: Option<VideoTarget>,
    audio: Option<AudioTarget>,
    subtitles: bool,
    progress: Option<Box<dyn FnMut(f32) -> Flow>>,
}

impl Converting {
    pub fn video(mut self, target: VideoTarget) -> Self {
        self.video = Some(target);
        self
    }

    pub fn audio(mut self, target: AudioTarget) -> Self {
        self.audio = Some(target);
        self
    }

    pub fn subtitles(mut self) -> Self {
        self.subtitles = true;
        self
    }

    pub fn on_progress(mut self, progress: impl FnMut(f32) -> Flow + 'static) -> Self {
        self.progress = Some(Box::new(progress));
        self
    }

    pub fn run(self) -> Result<Converted> {
        init::ensure()?;
        if same_file(&self.source, &self.destination) {
            return Err(Error::OverSource);
        }
        let mut source = Source::open(&self.source)?;
        let mut output = OutputContext::create(&self.destination)?;
        unsafe {
            let copied =
                avutil::av_dict_copy(&raw mut (*output.as_ptr()).metadata, source.metadata(), 0);
            check("av_dict_copy", copied)?;
        }
        let (mut lanes, converted) = self.plan(&source, &output)?;
        output.write_header(output.is_quicktime())?;
        for lane in &mut lanes {
            lane.update_time_base(&output);
        }

        let mut progress = self.progress;
        let mut packet = Packet::new()?;
        while source.read(&mut packet)? {
            let lane = usize::try_from(packet.stream_index())
                .ok()
                .and_then(|index| lanes.get_mut(index));
            if let (Some(progress), Some(fraction)) =
                (progress.as_mut(), progress_of(&source, &packet))
                && progress(fraction) == Flow::Cancel
            {
                return Err(Error::Cancelled);
            }
            match lane {
                Some(Lane::Copy(copied)) => copied.write(&mut packet, &mut output)?,
                Some(Lane::Video(lane)) => lane.feed(Some(&mut packet), &mut output)?,
                Some(Lane::Audio(lane)) => lane.feed(Some(&mut packet), &mut output)?,
                Some(Lane::Drop) | None => {}
            }
            packet.unref();
        }
        for lane in &mut lanes {
            match lane {
                Lane::Video(lane) => lane.feed(None, &mut output)?,
                Lane::Audio(lane) => lane.feed(None, &mut output)?,
                Lane::Copy(_) | Lane::Drop => {}
            }
        }
        output.write_trailer()?;
        if let Some(progress) = progress.as_mut() {
            progress(1.0);
        }
        Ok(converted)
    }

    fn plan(&self, source: &Source, output: &OutputContext) -> Result<(Vec<Lane>, Converted)> {
        let keeps_everything = output.format_name().to_bytes() == b"matroska";
        let mut lanes = Vec::with_capacity(source.streams());
        let mut converted = Converted {
            video: None,
            audio: None,
            duration: source
                .duration_us
                .map(|micros| Duration::from_micros(micros.max(0) as u64)),
        };

        for index in 0..source.streams() {
            let stream = source.stream(index);
            let accepted = output.accepts(unsafe { (*stream.codecpar).codec_id });
            let reuses = |reencode: bool, fits: bool, codec: &str| {
                let copyable = !reencode && fits && accepted;
                (
                    copyable && (keeps_everything || stream.codec_name() == codec),
                    copyable && keeps_everything,
                )
            };
            let lane = match (stream.kind(), &self.video, &self.audio) {
                (avutil::AVMEDIA_TYPE_VIDEO, Some(target), _) if !stream.is_picture() => {
                    let (copies, keeps) = reuses(
                        target.reencode,
                        fits(stream, target),
                        target.codec.stream_name(),
                    );
                    match (source.video == Some(index), copies) {
                        (true, true) => {
                            converted.video = Some(Handling::Copied);
                            Lane::Copy(CopiedStream::open(stream, output, source.start_us)?)
                        }
                        (true, false) => {
                            converted.video = Some(Handling::Encoded);
                            Lane::Video(Box::new(VideoLane::open(
                                stream,
                                output,
                                target,
                                source.start_us,
                            )?))
                        }
                        (false, _) if keeps => {
                            Lane::Copy(CopiedStream::open(stream, output, source.start_us)?)
                        }
                        (false, _) => Lane::Drop,
                    }
                }
                (avutil::AVMEDIA_TYPE_AUDIO, _, Some(target)) => {
                    let (copies, keeps) = reuses(target.reencode, true, target.codec.stream_name());
                    match (source.audio == Some(index), copies) {
                        (true, true) => {
                            converted.audio = Some(Handling::Copied);
                            Lane::Copy(CopiedStream::open(stream, output, source.start_us)?)
                        }
                        (true, false) => {
                            converted.audio = Some(Handling::Encoded);
                            Lane::Audio(Box::new(AudioLane::open(
                                stream,
                                output,
                                target,
                                source.start_us,
                            )?))
                        }
                        (false, _) if keeps => {
                            Lane::Copy(CopiedStream::open(stream, output, source.start_us)?)
                        }
                        (false, _) => Lane::Drop,
                    }
                }
                (avutil::AVMEDIA_TYPE_SUBTITLE, _, _) if self.subtitles && accepted => {
                    Lane::Copy(CopiedStream::open(stream, output, source.start_us)?)
                }
                _ => Lane::Drop,
            };
            lanes.push(lane);
        }

        if converted.video.is_none() && converted.audio.is_none() {
            return Err(Error::NothingToWrite);
        }
        Ok((lanes, converted))
    }
}

impl std::fmt::Debug for Converting {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Converting")
            .field("source", &self.source)
            .field("destination", &self.destination)
            .field("video", &self.video)
            .field("audio", &self.audio)
            .field("subtitles", &self.subtitles)
            .finish_non_exhaustive()
    }
}

enum Lane {
    Drop,
    Copy(CopiedStream),
    Video(Box<VideoLane>),
    Audio(Box<AudioLane>),
}

impl Lane {
    fn update_time_base(&mut self, output: &OutputContext) {
        match self {
            Lane::Copy(copied) => copied.update_time_base(output),
            Lane::Video(lane) => lane.update_time_base(output),
            Lane::Audio(_) | Lane::Drop => {}
        }
    }
}

fn fits(stream: &rikka_ffmpeg::avformat::AVStream, target: &VideoTarget) -> bool {
    let (width, height) = unsafe { ((*stream.codecpar).width, (*stream.codecpar).height) };
    target.bounds.is_none_or(|(most_width, most_height)| {
        width.max(0) as u32 <= most_width && height.max(0) as u32 <= most_height
    })
}

fn progress_of(source: &Source, packet: &Packet) -> Option<f32> {
    let duration = source.duration_us?;
    let stamp = packet.stamp()?;
    let index = usize::try_from(packet.stream_index()).ok()?;
    let time_base = source.stream(index).time_base;
    let micros = unsafe { avutil::av_rescale_q(stamp, time_base, avutil::AV_TIME_BASE_Q) };
    Some(((micros - source.start_us) as f32 / duration as f32).clamp(0.0, 1.0))
}

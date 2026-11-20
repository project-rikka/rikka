use std::path::{Path, PathBuf};
use std::time::Duration;

use rikka_ffmpeg::avutil;
use rikka_ffmpeg::{Frame, Packet};
use rikka_image::Image;
use rikka_types::{Flow, PixelSize};

use crate::error::{Error, Result};
use crate::shaping::{Shaping, orientation};
use rikka_video_core::decode::{Decoded, Decoder};
use rikka_video_core::demux::Demuxer;
use rikka_video_core::init;
use rikka_video_core::sw::SwConverter;

const DEFAULT_STEP: Duration = Duration::from_millis(100);

#[derive(Clone, Debug)]
#[must_use]
pub struct Sampling {
    path: PathBuf,
    bounds: Option<(u32, u32)>,
    step: Duration,
}

pub fn sample(path: impl AsRef<Path>) -> Sampling {
    Sampling {
        path: path.as_ref().to_path_buf(),
        bounds: None,
        step: DEFAULT_STEP,
    }
}

impl Sampling {
    pub fn with_max_size(mut self, size: PixelSize) -> Sampling {
        self.bounds = Some((size.width.max(1), size.height.max(1)));
        self
    }

    pub fn every(mut self, step: Duration) -> Sampling {
        self.step = step.max(Duration::from_millis(1));
        self
    }

    pub fn each(self, mut take: impl FnMut(Duration, Image) -> Flow) -> Result<()> {
        init::ensure()?;
        let mut demuxer = Demuxer::open(&self.path)?;
        demuxer.read_only_video();
        let stream = demuxer.stream();
        let shaping = Shaping {
            stored_aspect: stream.sample_aspect_ratio,
            orientation: orientation::from_stream(stream),
            bounds: self.bounds,
        };
        let time_base = demuxer.time_base();
        let origin = demuxer.start_us().unwrap_or(0);
        let mut decoder = Decoder::open(demuxer.decoder(), stream.codecpar, time_base, None)?;
        let mut converter = SwConverter::new();
        let mut packet = Packet::new()?;
        let mut decoded = Frame::new()?;
        let mut last_frame = Frame::new()?;
        let mut held_at: Option<Duration> = None;
        let mut due = Duration::ZERO;
        let mut draining = false;

        loop {
            match decoder.receive(&mut decoded)? {
                Decoded::Frame => {
                    let at = decoded
                        .pts_us(time_base)
                        .map_or(due, |micros| micros_after(micros, origin));
                    while held_at.is_some() && due < at {
                        let picture = shaping.picture(&mut converter, &last_frame)?;
                        if take(due, picture) == Flow::Cancel {
                            return Err(Error::Cancelled);
                        }
                        due += self.step;
                    }
                    last_frame.hold(&decoded)?;
                    held_at = Some(at);
                    decoded.unref();
                }
                Decoded::NeedsInput if draining => break,
                Decoded::NeedsInput => {
                    if demuxer.read(&mut packet)? {
                        let sent = decoder.send(Some(&mut packet));
                        packet.unref();
                        if let Err(error) = sent {
                            tracing::debug!("skipping corrupt video packet: {error}");
                        }
                    } else {
                        decoder.send(None)?;
                        draining = true;
                    }
                }
                Decoded::Eof => break,
            }
        }

        while held_at.is_some_and(|last| due <= last) {
            let picture = shaping.picture(&mut converter, &last_frame)?;
            if take(due, picture) == Flow::Cancel {
                return Err(Error::Cancelled);
            }
            due += self.step;
        }
        Ok(())
    }
}

fn micros_after(micros: i64, origin: i64) -> Duration {
    match micros.checked_sub(origin) {
        Some(after) if after > 0 && micros != avutil::AV_NOPTS_VALUE => {
            Duration::from_micros(after as u64)
        }
        _ => Duration::ZERO,
    }
}

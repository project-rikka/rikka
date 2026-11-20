use std::path::Path;
use std::time::Duration;

use rikka_image::Image;
use rikka_types::PixelSize;

use crate::error::{Error, Result};
use crate::read::VideoInfo;
use crate::shaping::{Shaping, orientation};
use crate::source::Source;
use rikka_ffmpeg::Frame;
use rikka_video_core::init;
use rikka_video_core::sw::SwConverter;

const FORWARD_REACH: Duration = Duration::from_secs(1);
const FIRST_BACKOFF: Duration = Duration::from_secs(1);
const LONGEST_BACKOFF: Duration = Duration::from_secs(16);

#[derive(Clone, Debug)]
pub struct Still {
    pub pts: Duration,
    pub image: Image,
}

pub struct Stills {
    source: Source,
    info: VideoInfo,
    shaping: Shaping,
    converter: SwConverter,
    current: Frame,
    scratch: Frame,
    position: Position,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Position {
    Opening,
    Sought,
    Ahead,
    Holding { pts: Duration, end: Duration },
    Ended(Duration),
}

impl Stills {
    pub fn open(path: impl AsRef<Path>) -> Result<Stills> {
        init::ensure()?;
        let (source, info) = Source::for_stills(path.as_ref())?;
        let stream = source.stream();
        let shaping = Shaping {
            stored_aspect: stream.sample_aspect_ratio,
            orientation: orientation::from_stream(stream),
            bounds: None,
        };
        Ok(Stills {
            source,
            info,
            shaping,
            converter: SwConverter::new(),
            current: Frame::new()?,
            scratch: Frame::new()?,
            position: Position::Opening,
        })
    }

    #[must_use]
    pub fn with_max_size(mut self, size: PixelSize) -> Stills {
        self.set_max_size(size);
        self
    }

    pub fn set_max_size(&mut self, size: PixelSize) {
        self.shaping.bounds = Some((size.width.max(1), size.height.max(1)));
    }

    #[must_use]
    pub fn info(&self) -> VideoInfo {
        self.info
    }

    pub fn still(&mut self, at: Duration, reach: Duration) -> Result<Still> {
        match self.position {
            Position::Holding { pts, end } if pts <= at && at < end => return self.show(pts),
            Position::Ended(pts) if pts <= at => return self.show(pts),
            _ => {}
        }
        if !self.can_decode_forward_to(at) {
            let near = self
                .source
                .keyframe_after(at)
                .is_some_and(|keyframe| keyframe < at.saturating_add(reach));
            match near && self.source.seek_ahead(at) {
                true => {
                    self.current.unref();
                    self.position = Position::Ahead;
                }
                false => self.seek(at)?,
            }
        }
        self.decode_until(at, reach, FIRST_BACKOFF)
    }

    fn can_decode_forward_to(&self, at: Duration) -> bool {
        let Position::Holding {
            pts: current_pts, ..
        } = self.position
        else {
            return false;
        };
        current_pts <= at
            && (at <= current_pts.saturating_add(FORWARD_REACH)
                || self
                    .source
                    .keyframe_before(at)
                    .is_none_or(|keyframe| keyframe <= current_pts))
    }

    fn seek(&mut self, to: Duration) -> Result<()> {
        self.current.unref();
        let landed = self.source.seek(to)?;
        self.position = match landed && !to.is_zero() {
            true => Position::Sought,
            false => Position::Opening,
        };
        Ok(())
    }

    fn decode_until(&mut self, at: Duration, reach: Duration, backoff: Duration) -> Result<Still> {
        let beyond = at.saturating_add(reach);
        loop {
            let Some(pts) = self.source.next(&mut self.scratch)? else {
                match self.position {
                    Position::Holding {
                        pts: current_pts, ..
                    } => {
                        self.position = Position::Ended(current_pts);
                        return self.show(current_pts);
                    }
                    Position::Ahead => {
                        self.seek(at)?;
                        return self.decode_until(at, reach, backoff);
                    }
                    _ => return Err(Error::Unsupported("video has no frames".into())),
                }
            };
            let end = pts.saturating_add(self.source.length(&self.scratch));
            match self.position {
                Position::Ahead if pts >= beyond => {
                    self.seek(at)?;
                    return self.decode_until(at, reach, backoff);
                }
                Position::Sought if pts > at && pts >= beyond && backoff <= LONGEST_BACKOFF => {
                    self.seek(at.saturating_sub(backoff))?;
                    return self.decode_until(at, reach, backoff * 2);
                }
                Position::Holding {
                    pts: current_pts, ..
                } if pts > at => {
                    let still = self.show(current_pts);
                    self.keep(pts, end);
                    return still;
                }
                _ => {
                    self.keep(pts, end);
                    if at < end || pts >= at {
                        return self.show(pts);
                    }
                }
            }
        }
    }

    fn keep(&mut self, pts: Duration, end: Duration) {
        std::mem::swap(&mut self.current, &mut self.scratch);
        self.scratch.unref();
        self.position = Position::Holding { pts, end };
    }

    fn show(&mut self, pts: Duration) -> Result<Still> {
        let image = self.shaping.picture(&mut self.converter, &self.current)?;
        Ok(Still { pts, image })
    }
}

impl std::fmt::Debug for Stills {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Stills")
            .field("info", &self.info)
            .field("position", &self.position)
            .finish_non_exhaustive()
    }
}

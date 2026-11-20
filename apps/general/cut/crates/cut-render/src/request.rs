use std::sync::Arc;
use std::time::{Duration, Instant};

use cut_project::{Project, Viewpoint};

use crate::overrides::Overrides;

const MIN_SCALE: f32 = 1.0 / 64.0;
const MAX_SCALE: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Playing {
    pub from: f64,
    pub since: Instant,
    pub speed: f64,
}

impl Playing {
    #[must_use]
    pub fn frame(&self, at: Instant, per_second: f64) -> f64 {
        let elapsed = match at.checked_duration_since(self.since) {
            Some(elapsed) => elapsed.as_secs_f64(),
            None => -self.since.duration_since(at).as_secs_f64(),
        };
        (self.from + elapsed * per_second * self.speed).max(0.0)
    }

    pub(crate) fn reaching(&self, frame: f64, per_second: f64) -> Option<Instant> {
        let rate = per_second * self.speed;
        if rate == 0.0 || !rate.is_finite() {
            return None;
        }
        let seconds = (frame - self.from) / rate;
        match seconds >= 0.0 {
            true => self
                .since
                .checked_add(Duration::try_from_secs_f64(seconds).ok()?),
            false => self
                .since
                .checked_sub(Duration::try_from_secs_f64(-seconds).ok()?),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Request {
    project: Arc<Project>,
    frame: i64,
    scale: f32,
    size: Option<(u32, u32)>,
    viewpoint: Viewpoint,
    overrides: Overrides,
    playing: Option<Playing>,
    rolling: bool,
    media_round: u64,
}

impl PartialEq for Request {
    fn eq(&self, other: &Request) -> bool {
        Arc::ptr_eq(&self.project, &other.project)
            && (self.playing.is_some() || self.frame == other.frame)
            && self.scale == other.scale
            && self.size == other.size
            && self.viewpoint == other.viewpoint
            && self.playing == other.playing
            && self.rolling == other.rolling
            && self.media_round == other.media_round
            && self.overrides == other.overrides
    }
}

impl Request {
    #[must_use]
    pub fn new(project: Arc<Project>, frame: i64) -> Request {
        Request {
            project,
            frame: frame.max(0),
            scale: 1.0,
            size: None,
            viewpoint: Viewpoint::Output,
            overrides: Overrides::default(),
            playing: None,
            rolling: false,
            media_round: 0,
        }
    }

    #[must_use]
    pub fn scale(mut self, scale: f32) -> Request {
        self.scale = match scale.is_finite() {
            true => scale.clamp(MIN_SCALE, MAX_SCALE),
            false => 1.0,
        };
        self.size = None;
        self
    }

    #[must_use]
    pub fn sized(self, width: u32, height: u32) -> Request {
        let format = self.project.format();
        let (width, height) = (width.max(1), height.max(1));
        let across = width as f32 / format.width.max(1) as f32;
        let down = height as f32 / format.height.max(1) as f32;
        let mut sized = self.scale(across.max(down));
        sized.size = Some((width, height));
        sized
    }

    #[must_use]
    pub fn viewpoint(mut self, viewpoint: Viewpoint) -> Request {
        self.viewpoint = viewpoint;
        self
    }

    #[must_use]
    pub fn overrides(mut self, overrides: Overrides) -> Request {
        self.overrides = overrides;
        self
    }

    #[must_use]
    pub fn playing(mut self, playing: Option<Playing>) -> Request {
        self.playing = playing;
        self
    }

    #[must_use]
    pub fn media_round(mut self, round: u64) -> Request {
        self.media_round = round;
        self
    }

    pub(crate) fn round(&self) -> u64 {
        self.media_round
    }

    #[must_use]
    pub fn rolling(mut self) -> Request {
        self.rolling = true;
        self
    }

    #[must_use]
    pub(crate) fn seen_from(&self) -> Viewpoint {
        self.viewpoint
    }

    #[must_use]
    pub fn project(&self) -> &Arc<Project> {
        &self.project
    }

    #[must_use]
    pub fn frame(&self) -> i64 {
        self.frame
    }

    #[must_use]
    pub fn output(&self) -> (u32, u32) {
        if let Some(size) = self.size {
            return size;
        }
        let format = self.project.format();
        let side = |length: u32| ((length as f32 * self.scale).ceil() as u32).max(1);
        (side(format.width), side(format.height))
    }

    pub(crate) fn factor(&self) -> f32 {
        self.scale
    }

    pub(crate) fn changes(&self) -> &Overrides {
        &self.overrides
    }

    pub(crate) fn clock(&self) -> Option<Playing> {
        self.playing
    }

    pub(crate) fn forward(&self) -> f64 {
        match self.playing {
            Some(playing) => playing.speed.max(0.0),
            None if self.rolling => 1.0,
            None => 0.0,
        }
    }

    pub(crate) fn per_second(&self) -> f64 {
        self.project.rate().per_second()
    }

    pub(crate) fn due(&self, at: Instant) -> i64 {
        let Some(playing) = self.playing else {
            return self.frame;
        };
        let end = self.project.end();
        let frame = playing.frame(at, self.per_second()).floor() as i64;
        frame.clamp(0, end.max(0))
    }
}

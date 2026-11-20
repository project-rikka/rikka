use std::time::Instant;

use rikka_animation::{Channel, Curve, Presence, Property};
use rikka_types::{Color, CornerRadii, Point, Rect, Size};
use smallvec::{Array, SmallVec};

use crate::motion::track::Track;

pub trait Animatable: Copy + PartialEq + 'static {
    const PROPERTY: Property;

    fn channels(self) -> [f32; 4];

    fn from_channels(channels: [f32; 4]) -> Self;
}

impl Animatable for f32 {
    const PROPERTY: Property = Property::Spatial;

    fn channels(self) -> [f32; 4] {
        [self, 0.0, 0.0, 0.0]
    }

    fn from_channels([value, ..]: [f32; 4]) -> Self {
        value
    }
}

impl Animatable for Point {
    const PROPERTY: Property = Property::Spatial;

    fn channels(self) -> [f32; 4] {
        [self.x, self.y, 0.0, 0.0]
    }

    fn from_channels([x, y, ..]: [f32; 4]) -> Self {
        Point::new(x, y)
    }
}

impl Animatable for Size {
    const PROPERTY: Property = Property::Spatial;

    fn channels(self) -> [f32; 4] {
        [self.width, self.height, 0.0, 0.0]
    }

    fn from_channels([width, height, ..]: [f32; 4]) -> Self {
        Size::new(width, height)
    }
}

impl Animatable for Rect {
    const PROPERTY: Property = Property::Spatial;

    fn channels(self) -> [f32; 4] {
        [self.x(), self.y(), self.width(), self.height()]
    }

    fn from_channels([x, y, width, height]: [f32; 4]) -> Self {
        Rect::new(x, y, width, height)
    }
}

impl Animatable for CornerRadii {
    const PROPERTY: Property = Property::Spatial;

    fn channels(self) -> [f32; 4] {
        [
            self.top_left,
            self.top_right,
            self.bottom_right,
            self.bottom_left,
        ]
    }

    fn from_channels([top_left, top_right, bottom_right, bottom_left]: [f32; 4]) -> Self {
        CornerRadii::new(top_left, top_right, bottom_right, bottom_left)
    }
}

impl Animatable for Color {
    const PROPERTY: Property = Property::Effects;

    fn channels(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    fn from_channels([r, g, b, a]: [f32; 4]) -> Self {
        Color::rgba(r, g, b, a)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Clock {
    pub(crate) now: Instant,
    pub(crate) began: Instant,
}

type Channels = [Channel; 4];

fn channels<T: Animatable>(value: T, last: Instant) -> Track<Channels> {
    Track::new(value.channels().map(Channel::new), Some(last))
}

fn position<C>(tracks: &[(u64, Track<C>)], key: u64) -> Option<usize> {
    tracks.iter().position(|(entry_key, _)| *entry_key == key)
}

fn entry<C, A: Array<Item = (u64, Track<C>)>>(
    tracks: &mut SmallVec<A>,
    key: u64,
    start: impl FnOnce() -> Track<C>,
) -> &mut Track<C> {
    let at = match position(tracks, key) {
        Some(at) => at,
        None => {
            tracks.push((key, start()));
            tracks.len() - 1
        }
    };
    &mut tracks[at].1
}

fn take<C, A: Array<Item = (u64, Track<C>)>>(
    tracks: &mut SmallVec<A>,
    key: u64,
) -> Option<Track<C>> {
    let at = position(tracks, key)?;
    Some(tracks.remove(at).1)
}

const INLINE_TRACKS: usize = 1;

#[derive(Default)]
pub(crate) struct Motions {
    tracks: SmallVec<[(u64, Track<Channels>); INLINE_TRACKS]>,
    presences: SmallVec<[(u64, Track<Presence>); 0]>,
}

pub(crate) const fn motion_key(name: &str) -> u64 {
    let bytes = name.as_bytes();
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut at = 0;
    while at < bytes.len() {
        hash ^= bytes[at] as u64;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
        at += 1;
    }
    hash
}

impl Motions {
    fn find(&self, key: u64) -> Option<&Track<Channels>> {
        position(&self.tracks, key).map(|at| &self.tracks[at].1)
    }

    fn find_mut(&mut self, key: u64) -> Option<&mut Track<Channels>> {
        position(&self.tracks, key).map(|at| &mut self.tracks[at].1)
    }

    pub(crate) fn drive<T: Animatable>(
        &mut self,
        key: u64,
        start: T,
        target: T,
        curve: Curve,
        clock: Clock,
    ) -> (T, bool) {
        let track = entry(&mut self.tracks, key, || channels(start, clock.began));
        let elapsed = track.elapsed(clock);
        let mut moving = false;
        for (channel, goal) in track.course.iter_mut().zip(target.channels()) {
            channel.advance(curve, goal, elapsed);
            moving |= !channel.is_resting();
        }
        (
            T::from_channels(track.course.map(|channel| channel.value)),
            moving,
        )
    }

    pub(crate) fn present(
        &mut self,
        key: u64,
        shown: bool,
        curve: Curve,
        clock: Clock,
    ) -> (f32, bool) {
        let placed = match position(&self.presences, key) {
            Some(_) => None,
            None => take(&mut self.tracks, key),
        };
        let track = entry(&mut self.presences, key, || match placed {
            Some(placed) => {
                let [channel, ..] = placed.course;
                Track::new(
                    Presence::new(channel.value, channel.velocity),
                    placed.last(),
                )
            }
            None => Track::new(Presence::HIDDEN, None),
        });
        let elapsed = track.elapsed(clock);
        track.course.advance(shown, curve, elapsed);
        (track.course.progress(), !track.course.is_resting())
    }

    pub(crate) fn jump<T: Animatable>(&mut self, key: u64, value: T, clock: Clock) {
        self.place(key, value, clock.now);
    }

    pub(crate) fn fling<T: Animatable>(&mut self, key: u64, value: T, velocity: T, clock: Clock) {
        self.place(key, value, clock.began);
        if let Some(track) = self.find_mut(key) {
            for (channel, speed) in track.course.iter_mut().zip(velocity.channels()) {
                channel.velocity = speed;
            }
        }
    }

    fn place<T: Animatable>(&mut self, key: u64, value: T, last: Instant) {
        take(&mut self.presences, key);
        let track = channels(value, last);
        match self.find_mut(key) {
            Some(entry) => *entry = track,
            None => self.tracks.push((key, track)),
        }
    }

    pub(crate) fn impel<T: Animatable>(&mut self, key: u64, velocity: T, clock: Clock) {
        let Some(track) = self.find_mut(key) else {
            return;
        };
        track.wake(clock);
        for (channel, speed) in track.course.iter_mut().zip(velocity.channels()) {
            channel.velocity += speed;
        }
    }

    pub(crate) fn shift<T: Animatable>(&mut self, key: u64, by: T) {
        if let Some(track) = self.find_mut(key) {
            for (channel, by) in track.course.iter_mut().zip(by.channels()) {
                channel.shift(by);
            }
        }
    }

    pub(crate) fn target<T: Animatable>(&self, key: u64) -> Option<T> {
        let track = self.find(key)?;
        Some(T::from_channels(
            track.course.map(|channel| channel.target()),
        ))
    }

    pub(crate) fn holds(&self, key: u64) -> bool {
        position(&self.tracks, key).is_some() || position(&self.presences, key).is_some()
    }

    pub(crate) fn forget(&mut self, key: u64) {
        take(&mut self.tracks, key);
        take(&mut self.presences, key);
    }
}

impl std::fmt::Debug for Motions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Motions")
            .field("tracks", &self.tracks.len())
            .field("presences", &self.presences.len())
            .finish()
    }
}

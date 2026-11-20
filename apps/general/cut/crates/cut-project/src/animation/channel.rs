use std::ops::Range;

use rikka_keyframe::Easing;

use super::graph::Span;
use super::key::{Bend, Handle, Interpolation, Key, Preset, Side, Smooth, Timing};
use super::kind::{Chroma, Kind, Planar, Scalar, Spatial, Value};
use super::track::Track;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Channel<'a> {
    Scalar(&'a Track<Scalar>),
    Planar(&'a Track<Planar>),
    Spatial(&'a Track<Spatial>),
    Chroma(&'a Track<Chroma>),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Tracked {
    Scalar(Track<Scalar>),
    Planar(Track<Planar>),
    Spatial(Track<Spatial>),
    Chroma(Track<Chroma>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum AnyKey {
    Scalar(Key<Scalar>),
    Planar(Key<Planar>),
    Spatial(Key<Spatial>),
    Chroma(Key<Chroma>),
}

macro_rules! each {
    ($value:expr, $track:ident => $body:expr) => {
        match $value {
            Channel::Scalar($track) => $body,
            Channel::Planar($track) => $body,
            Channel::Spatial($track) => $body,
            Channel::Chroma($track) => $body,
        }
    };
}

macro_rules! each_tracked {
    ($value:expr, $track:ident => $body:expr) => {
        match $value {
            Tracked::Scalar($track) => $body,
            Tracked::Planar($track) => $body,
            Tracked::Spatial($track) => $body,
            Tracked::Chroma($track) => $body,
        }
    };
}
macro_rules! rebuilt {
    ($value:expr, $track:ident => $body:expr) => {
        match $value {
            Tracked::Scalar($track) => Tracked::Scalar($body),
            Tracked::Planar($track) => Tracked::Planar($body),
            Tracked::Spatial($track) => Tracked::Spatial($body),
            Tracked::Chroma($track) => Tracked::Chroma($body),
        }
    };
}

macro_rules! key {
    ($value:expr, $key:ident => $body:expr) => {
        match $value {
            AnyKey::Scalar($key) => $body,
            AnyKey::Planar($key) => $body,
            AnyKey::Spatial($key) => $body,
            AnyKey::Chroma($key) => $body,
        }
    };
}

impl Channel<'_> {
    #[must_use]
    pub fn frames(&self) -> Vec<i64> {
        each!(self, track => track.keys().iter().map(|key| key.at).collect())
    }

    #[must_use]
    pub fn has_key(&self, at: i64) -> bool {
        each!(self, track => track.key(at).is_some())
    }

    #[must_use]
    pub fn timing(&self, at: i64) -> Option<Timing> {
        each!(self, track => track.key(at).map(|key| key.timing))
    }

    #[must_use]
    pub fn bend(&self, at: i64) -> Option<Bend> {
        match self {
            Channel::Spatial(track) => track.key(at).map(|key| key.bend),
            Channel::Scalar(_) | Channel::Planar(_) | Channel::Chroma(_) => None,
        }
    }

    #[must_use]
    pub fn value(&self, at: i64) -> Option<Value> {
        each!(self, track => track.key(at).map(|key| erase(track, key.value)))
    }

    #[must_use]
    pub fn sample(&self, at: f64) -> Option<Value> {
        each!(self, track => track.sample(at).map(|value| erase(track, value)))
    }

    #[must_use]
    pub fn speed(&self, at: f64) -> Option<f64> {
        each!(self, track => track.speed(at))
    }

    #[must_use]
    pub fn graph_value(&self, at: f64) -> Option<f64> {
        each!(self, track => track.graph_value(at))
    }

    #[must_use]
    pub fn spans(&self) -> Vec<Span> {
        each!(self, track => track.spans())
    }

    #[must_use]
    pub fn handle_toward(&self, at: i64, side: Side, point: [f64; 2]) -> Option<Handle> {
        each!(self, track => track.handle_toward(at, side, point))
    }

    pub(crate) fn any_key(&self, at: i64) -> Option<AnyKey> {
        match self {
            Channel::Scalar(track) => track.key(at).copied().map(AnyKey::Scalar),
            Channel::Planar(track) => track.key(at).copied().map(AnyKey::Planar),
            Channel::Spatial(track) => track.key(at).copied().map(AnyKey::Spatial),
            Channel::Chroma(track) => track.key(at).copied().map(AnyKey::Chroma),
        }
    }
}

fn erase<K: Kind>(_: &Track<K>, value: K::Value) -> Value {
    K::erase(value)
}

impl Tracked {
    pub(crate) fn channel(&self) -> Channel<'_> {
        match self {
            Tracked::Scalar(track) => Channel::Scalar(track),
            Tracked::Planar(track) => Channel::Planar(track),
            Tracked::Spatial(track) => Channel::Spatial(track),
            Tracked::Chroma(track) => Channel::Chroma(track),
        }
    }

    pub(crate) fn empty_like(value: Value, spatial: bool) -> Tracked {
        match (value, spatial) {
            (Value::Scalar(_), _) => Tracked::Scalar(Track::default()),
            (Value::Point(_), true) => Tracked::Spatial(Track::default()),
            (Value::Point(_), false) => Tracked::Planar(Track::default()),
            (Value::Color(_), _) => Tracked::Chroma(Track::default()),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        each_tracked!(self, track => track.is_empty())
    }

    pub(crate) fn frames(&self) -> Vec<i64> {
        self.channel().frames()
    }

    pub(crate) fn sample(&self, at: f64) -> Option<Value> {
        self.channel().sample(at)
    }

    pub(crate) fn first(&self) -> Option<Value> {
        each_tracked!(self, track => track.keys().first().map(|key| erase(track, key.value)))
    }

    pub(crate) fn has_key(&self, at: i64) -> bool {
        self.channel().has_key(at)
    }

    pub(crate) fn set(&mut self, at: i64, value: Value) -> bool {
        each_tracked!(self, track => restored(track, value).map(|value| track.set(at, value)).is_some())
    }

    pub(crate) fn remove(&mut self, at: i64) -> Option<Value> {
        each_tracked!(self, track => track.remove(at).map(|key| erase(track, key.value)))
    }

    pub(crate) fn interpolate(&mut self, at: i64, interpolation: Interpolation) -> bool {
        each_tracked!(self, track => track.interpolate(at, interpolation))
    }

    pub(crate) fn smooth(&mut self, at: i64, smooth: Smooth) -> bool {
        each_tracked!(self, track => track.smooth(at, smooth))
    }

    pub(crate) fn steer(&mut self, at: i64, side: Side, handle: Handle) -> bool {
        each_tracked!(self, track => track.steer(at, side, handle))
    }

    pub(crate) fn rebend(&mut self, at: i64, bend: Bend) -> bool {
        match self {
            Tracked::Spatial(track) => track.change(at, |key| key.bend = bend),
            Tracked::Scalar(_) | Tracked::Planar(_) | Tracked::Chroma(_) => false,
        }
    }

    pub(crate) fn map(&mut self, change: impl Fn(Value) -> Value) {
        each_tracked!(self, track => map_values(track, change))
    }

    pub(crate) fn cropped(&self, span: &Range<i64>) -> Tracked {
        rebuilt!(self, track => track.cropped(span))
    }

    pub(crate) fn tween_preset(&self, last: i64) -> Option<Preset> {
        each_tracked!(self, track => track.tween_preset(last))
    }

    pub(crate) fn stretched(&self, last: i64) -> Tracked {
        rebuilt!(self, track => track.stretched(last))
    }

    pub(crate) fn spanning(&self, last: i64) -> Tracked {
        match last {
            0 => self.cropped(&(0..1)),
            _ => self.stretched(last),
        }
    }

    pub(crate) fn eased(&self, easing: Easing) -> Tracked {
        rebuilt!(self, track => track.eased(easing))
    }

    pub(crate) fn reversed(&self) -> Tracked {
        rebuilt!(self, track => track.reversed())
    }

    pub(crate) fn key(&self, at: i64) -> Option<AnyKey> {
        self.channel().any_key(at)
    }

    pub(crate) fn take(&mut self, frames: &[i64]) -> Vec<AnyKey> {
        match self {
            Tracked::Scalar(track) => track.take(frames).into_iter().map(AnyKey::Scalar).collect(),
            Tracked::Planar(track) => track.take(frames).into_iter().map(AnyKey::Planar).collect(),
            Tracked::Spatial(track) => track
                .take(frames)
                .into_iter()
                .map(AnyKey::Spatial)
                .collect(),
            Tracked::Chroma(track) => track.take(frames).into_iter().map(AnyKey::Chroma).collect(),
        }
    }

    pub(crate) fn put(&mut self, key: AnyKey) -> bool {
        match (self, key) {
            (Tracked::Scalar(track), AnyKey::Scalar(key)) => track.put(key),
            (Tracked::Planar(track), AnyKey::Planar(key)) => track.put(key),
            (Tracked::Spatial(track), AnyKey::Spatial(key)) => track.put(key),
            (Tracked::Chroma(track), AnyKey::Chroma(key)) => track.put(key),
            _ => return false,
        }
        true
    }
}

impl AnyKey {
    pub(crate) fn at(&self) -> i64 {
        key!(self, key => key.at)
    }

    pub(crate) fn moved(self, at: i64) -> AnyKey {
        match self {
            AnyKey::Scalar(key) => AnyKey::Scalar(Key { at, ..key }),
            AnyKey::Planar(key) => AnyKey::Planar(Key { at, ..key }),
            AnyKey::Spatial(key) => AnyKey::Spatial(Key { at, ..key }),
            AnyKey::Chroma(key) => AnyKey::Chroma(Key { at, ..key }),
        }
    }

    pub(crate) fn value(&self) -> Value {
        match self {
            AnyKey::Scalar(key) => Value::Scalar(key.value),
            AnyKey::Planar(key) => Value::Point(key.value),
            AnyKey::Spatial(key) => Value::Point(key.value),
            AnyKey::Chroma(key) => Value::Color(key.value),
        }
    }
}

fn restored<K: Kind>(_: &Track<K>, value: Value) -> Option<K::Value> {
    K::restore(value)
}

fn map_values<K: Kind>(track: &mut Track<K>, change: impl Fn(Value) -> Value) {
    track.map_values(|value| K::restore(change(K::erase(value))).unwrap_or(value));
}

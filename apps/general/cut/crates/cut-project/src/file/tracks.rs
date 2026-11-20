use std::collections::BTreeMap;

use rikka_keyframe::{Ease, Easing, Family};
use rikka_types::Point;
use serde::{Deserialize, Serialize};

use super::items::{restored_color, stored_color};
use crate::animation::{
    Bend, Chroma, Handle, Incoming, Key, Kind, Outgoing, Planar, Preset, Scalar, Smooth, Spatial,
    Timing, Track, Tracked, Value,
};
use crate::clip::Clip;
use crate::motion::Motion;
use crate::property::Field;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StoredKey {
    pub(super) at: i64,
    pub(super) value: StoredValue,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    incoming: Option<StoredHandle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    outgoing: Option<StoredOutgoing>,
    #[serde(default, skip_serializing_if = "StoredSmooth::is_manual")]
    smooth: StoredSmooth,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bend: Option<StoredBend>,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(untagged)]
pub(super) enum StoredValue {
    Scalar(f32),
    Pair([f32; 2]),
    Quad([f32; 4]),
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredHandle {
    speed: f32,
    influence: f32,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum StoredOutgoing {
    Hold,
    Bezier {
        speed: f32,
        influence: f32,
    },
    Preset {
        easing: StoredEasing,
        from: f32,
        to: f32,
    },
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StoredEasing {
    curve: StoredCurve,
    flow: StoredEaseMode,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StoredCurve {
    Linear,
    Sine,
    Quad,
    Cubic,
    Quart,
    Quint,
    Expo,
    Circ,
    Back,
    Elastic,
    Bounce,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StoredEaseMode {
    In,
    Out,
    InOut,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StoredSmooth {
    #[default]
    Manual,
    Continuous,
    Auto,
}

impl StoredSmooth {
    fn is_manual(&self) -> bool {
        *self == StoredSmooth::Manual
    }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredBend {
    incoming: [f32; 2],
    outgoing: [f32; 2],
    #[serde(default, skip_serializing_if = "StoredSmooth::is_manual")]
    smooth: StoredSmooth,
    #[serde(default, skip_serializing_if = "is_curved")]
    straight: bool,
}

fn is_curved(straight: &bool) -> bool {
    !*straight
}

pub(super) fn stored_easing(easing: Easing) -> StoredEasing {
    StoredEasing {
        curve: match easing.family {
            Family::Linear => StoredCurve::Linear,
            Family::Sine => StoredCurve::Sine,
            Family::Quad => StoredCurve::Quad,
            Family::Cubic => StoredCurve::Cubic,
            Family::Quart => StoredCurve::Quart,
            Family::Quint => StoredCurve::Quint,
            Family::Expo => StoredCurve::Expo,
            Family::Circ => StoredCurve::Circ,
            Family::Back => StoredCurve::Back,
            Family::Elastic => StoredCurve::Elastic,
            Family::Bounce => StoredCurve::Bounce,
        },
        flow: match easing.ease {
            Ease::In => StoredEaseMode::In,
            Ease::Out => StoredEaseMode::Out,
            Ease::InOut => StoredEaseMode::InOut,
        },
    }
}

pub(super) fn restored_easing(easing: StoredEasing) -> Easing {
    Easing::new(
        match easing.curve {
            StoredCurve::Linear => Family::Linear,
            StoredCurve::Sine => Family::Sine,
            StoredCurve::Quad => Family::Quad,
            StoredCurve::Cubic => Family::Cubic,
            StoredCurve::Quart => Family::Quart,
            StoredCurve::Quint => Family::Quint,
            StoredCurve::Expo => Family::Expo,
            StoredCurve::Circ => Family::Circ,
            StoredCurve::Back => Family::Back,
            StoredCurve::Elastic => Family::Elastic,
            StoredCurve::Bounce => Family::Bounce,
        },
        match easing.flow {
            StoredEaseMode::In => Ease::In,
            StoredEaseMode::Out => Ease::Out,
            StoredEaseMode::InOut => Ease::InOut,
        },
    )
}

fn stored_smooth(smooth: Smooth) -> StoredSmooth {
    match smooth {
        Smooth::Manual => StoredSmooth::Manual,
        Smooth::Continuous => StoredSmooth::Continuous,
        Smooth::Auto => StoredSmooth::Auto,
    }
}

fn restored_smooth(smooth: StoredSmooth) -> Smooth {
    match smooth {
        StoredSmooth::Manual => Smooth::Manual,
        StoredSmooth::Continuous => Smooth::Continuous,
        StoredSmooth::Auto => Smooth::Auto,
    }
}

fn stored_value(value: Value) -> StoredValue {
    match value {
        Value::Scalar(value) => StoredValue::Scalar(value),
        Value::Point(point) => StoredValue::Pair([point.x, point.y]),
        Value::Color(color) => StoredValue::Quad(stored_color(color)),
    }
}

fn stored_key<K: Kind>(key: &Key<K>) -> StoredKey {
    StoredKey {
        at: key.at,
        value: stored_value(K::erase(key.value)),
        incoming: match key.timing.incoming {
            Incoming::Linear => None,
            Incoming::Bezier(handle) => Some(StoredHandle {
                speed: handle.speed,
                influence: handle.influence,
            }),
        },
        outgoing: match key.timing.outgoing {
            Outgoing::Linear => None,
            Outgoing::Hold => Some(StoredOutgoing::Hold),
            Outgoing::Bezier(handle) => Some(StoredOutgoing::Bezier {
                speed: handle.speed,
                influence: handle.influence,
            }),
            Outgoing::Preset(preset) => Some(StoredOutgoing::Preset {
                easing: stored_easing(preset.easing),
                from: preset.from,
                to: preset.to,
            }),
        },
        smooth: stored_smooth(key.timing.smooth),
        bend: K::bend(&key.bend).map(|bend| StoredBend {
            incoming: [bend.incoming.x, bend.incoming.y],
            outgoing: [bend.outgoing.x, bend.outgoing.y],
            smooth: stored_smooth(bend.smooth),
            straight: bend.straight,
        }),
    }
}

pub(super) fn stored_track<K: Kind>(track: &Track<K>) -> Vec<StoredKey> {
    track.keys().iter().map(stored_key).collect()
}

pub(super) fn stored_motion(motion: &Motion) -> BTreeMap<String, Vec<StoredKey>> {
    motion
        .tracks()
        .iter()
        .filter(|(_, track)| !track.is_empty())
        .map(|(field, track)| {
            let keys = match track {
                Tracked::Scalar(track) => stored_track(track),
                Tracked::Planar(track) => stored_track(track),
                Tracked::Spatial(track) => stored_track(track),
                Tracked::Chroma(track) => stored_track(track),
            };
            (field.name().to_owned(), keys)
        })
        .collect()
}

pub(super) fn ordered(keys: &[StoredKey], length: i64) -> bool {
    keys.windows(2).all(|pair| pair[0].at < pair[1].at)
        && keys.iter().all(|key| (0..length).contains(&key.at))
}

fn restored_key<K: Kind>(
    key: &StoredKey,
    value: impl Fn(StoredValue) -> Option<K::Value>,
) -> Result<Key<K>, String> {
    let wrong = || format!("the key at {} holds the wrong kind of value", key.at);
    let parsed = value(key.value).ok_or_else(wrong)?;
    let handle = |speed: f32, influence: f32| match speed.is_finite() && influence.is_finite() {
        true => Ok(Handle::new(speed, influence).checked()),
        false => Err(format!("the key at {} is eased nowhere", key.at)),
    };
    let incoming = match key.incoming {
        None => Incoming::Linear,
        Some(stored) => Incoming::Bezier(handle(stored.speed, stored.influence)?),
    };
    let outgoing = match key.outgoing {
        None => Outgoing::Linear,
        Some(StoredOutgoing::Hold) => Outgoing::Hold,
        Some(StoredOutgoing::Bezier { speed, influence }) => {
            Outgoing::Bezier(handle(speed, influence)?)
        }
        Some(StoredOutgoing::Preset { easing, from, to }) => {
            let preset = Preset {
                easing: restored_easing(easing),
                from,
                to,
            };
            if !(from.is_finite() && to.is_finite() && 0.0 <= from && from < to && to <= 1.0) {
                return Err(format!("the key at {} eases over nothing", key.at));
            }
            Outgoing::Preset(preset)
        }
    };
    let mut bend = K::Bend::default();
    match (K::bend_mut(&mut bend), key.bend) {
        (Some(spatial), Some(stored)) => {
            let tangent = |[x, y]: [f32; 2]| Point::new(x, y);
            *spatial = Bend {
                incoming: tangent(stored.incoming),
                outgoing: tangent(stored.outgoing),
                smooth: restored_smooth(stored.smooth),
                straight: stored.straight,
            };
            if !spatial.is_finite() {
                return Err(format!("the key at {} bends nowhere", key.at));
            }
        }
        (None, Some(_)) => return Err(wrong()),
        (Some(_) | None, None) => {}
    }
    Ok(Key {
        at: key.at,
        value: parsed,
        timing: Timing {
            incoming,
            outgoing,
            smooth: restored_smooth(key.smooth),
        },
        bend,
    })
}

fn restored_keys<K: Kind>(
    keys: &[StoredKey],
    value: impl Fn(StoredValue) -> Option<K::Value>,
) -> Result<Track<K>, String> {
    let keys = keys
        .iter()
        .map(|key| restored_key::<K>(key, &value))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Track::new(keys))
}

pub(super) fn restored_scalar(keys: &[StoredKey]) -> Result<Track<Scalar>, String> {
    restored_keys::<Scalar>(keys, |value| match value {
        StoredValue::Scalar(value) if value.is_finite() => Some(value),
        _ => None,
    })
}

fn restored_tracked(like: Value, spatial: bool, keys: &[StoredKey]) -> Result<Tracked, String> {
    let point = |value: StoredValue| match value {
        StoredValue::Pair([x, y]) if x.is_finite() && y.is_finite() => Some(Point::new(x, y)),
        _ => None,
    };
    Ok(match (like, spatial) {
        (Value::Scalar(_), _) => Tracked::Scalar(restored_scalar(keys)?),
        (Value::Point(_), true) => Tracked::Spatial(restored_keys::<Spatial>(keys, point)?),
        (Value::Point(_), false) => Tracked::Planar(restored_keys::<Planar>(keys, point)?),
        (Value::Color(_), _) => {
            Tracked::Chroma(restored_keys::<Chroma>(keys, |value| match value {
                StoredValue::Quad(color) => restored_color(color).ok(),
                _ => None,
            })?)
        }
    })
}

pub(super) fn restored_motion(
    tracks: &BTreeMap<String, Vec<StoredKey>>,
    clip: &Clip,
    id: u64,
) -> Result<Motion, String> {
    let mut motion = Motion::default();
    for (name, keys) in tracks {
        let field = Field::named(name, clip.camera().is_some())
            .ok_or_else(|| format!("clip {id} animates an unknown “{name}”"))?;
        let like = clip
            .base(field)
            .ok_or_else(|| format!("clip {id} has no “{name}” to animate"))?;
        if !ordered(keys, clip.length()) {
            return Err(format!("the keys of clip {id} are out of order"));
        }
        let mut track = restored_tracked(like, field.is_spatial(), keys)
            .map_err(|reason| format!("clip {id}: {reason}"))?;
        track.map(|value| field.clamp_value(value));
        motion.insert(field, track);
    }
    Ok(motion)
}

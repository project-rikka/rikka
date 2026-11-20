use std::collections::BTreeMap;
use std::ops::Range;

use rikka_keyframe::Easing;

use crate::animation::{Channel, Interpolation, Tracked, Value};
use crate::placement::turned;
use crate::property::{Animated, Field};

#[derive(Clone, Debug)]
pub(crate) struct Tweening {
    pub(crate) fields: Vec<Field>,
    pub(crate) last: i64,
    pub(crate) easing: Easing,
}

impl Tweening {
    fn covers(&self, field: Field) -> bool {
        self.fields.contains(&field)
    }

    fn ends_at(&self, at: i64) -> bool {
        at == 0 || at == self.last
    }

    pub(crate) fn track(&self, field: Field, start: Value, end: Value) -> Tracked {
        let mut track = Tracked::empty_like(start, field.is_spatial());
        track.set(0, field.clamp_value(start));
        track.set(self.last, field.clamp_value(end));
        track.interpolate(0, Interpolation::Preset(self.easing));
        track
    }

    fn joined(&self, field: Field, at: i64, before: Value, after: Value) -> Tracked {
        match at {
            0 => self.track(field, after, before),
            _ => self.track(field, before, after),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Motion {
    tracks: BTreeMap<Field, Tracked>,
}

impl Motion {
    pub(crate) fn channel(&self, field: Field) -> Option<Channel<'_>> {
        self.tracks
            .get(&field)
            .filter(|track| !track.is_empty())
            .map(Tracked::channel)
    }

    pub(crate) fn is_still(&self) -> bool {
        self.tracks.values().all(Tracked::is_empty)
    }

    pub(crate) fn remapped(&self, change: impl Fn(Field, &Tracked) -> Tracked) -> Motion {
        Motion {
            tracks: self
                .tracks
                .iter()
                .map(|(field, track)| (*field, change(*field, track)))
                .filter(|(_, track)| !track.is_empty())
                .collect(),
        }
    }

    pub(crate) fn moves(&self) -> bool {
        self.tracks
            .iter()
            .any(|(field, track)| !field.is_audible() && !track.is_empty())
    }

    pub(crate) fn tracks(&self) -> &BTreeMap<Field, Tracked> {
        &self.tracks
    }

    pub(crate) fn take(&mut self, field: Field) -> Option<Tracked> {
        self.tracks.remove(&field)
    }

    pub(crate) fn insert(&mut self, field: Field, track: Tracked) {
        match track.is_empty() {
            true => self.tracks.remove(&field),
            false => self.tracks.insert(field, track),
        };
    }

    pub(crate) fn keep(&mut self, retain: impl Fn(Field) -> bool) {
        self.tracks.retain(|field, _| retain(*field));
    }

    pub(crate) fn frames(&self) -> impl Iterator<Item = i64> + '_ {
        self.tracks.values().flat_map(Tracked::frames)
    }

    pub(crate) fn animated<T: Animated>(&self, base: &T, at: f64) -> T {
        base.animated_fields()
            .into_iter()
            .fold(base.clone(), |result, field| {
                match self.tracks.get(&field).and_then(|track| track.sample(at)) {
                    Some(value) => result.with_field(field, field.clamp_value(value)),
                    None => result,
                }
            })
    }

    pub(crate) fn absorbed<T: Animated>(
        &mut self,
        base: &T,
        target: &T,
        at: i64,
        tweening: Option<&Tweening>,
    ) -> T {
        let current = self.animated(base, at as f64);
        let changes: Vec<(Field, Option<(Value, Value)>)> = target
            .animated_fields()
            .into_iter()
            .map(|field| (field, changed(&current, target, field)))
            .collect();
        let shifting = tweening.is_some_and(|tweening| {
            !tweening.ends_at(at)
                || changes
                    .iter()
                    .any(|(field, change)| field.is_pivot() && change.is_some())
        });
        let mut updated = target.clone();
        for (field, change) in changes {
            if let Some((before, after)) = change {
                let tweened = tweening.filter(|tweening| tweening.covers(field));
                let track = self
                    .tracks
                    .get_mut(&field)
                    .filter(|track| !track.is_empty());
                match (track, tweened) {
                    (Some(track), Some(_)) if shifting => {
                        track.map(|value| field.clamp_value(shifted(field, value, before, after)));
                    }
                    (Some(track), _) => {
                        let value = landed(field, track, at, before, after);
                        track.set(at, field.clamp_value(value));
                    }
                    (None, Some(tweening)) if !shifting => {
                        let joined = tweening.joined(field, at, before, after);
                        self.tracks.insert(field, joined);
                    }
                    (None, _) => {}
                }
            }
            if self.channel(field).is_some()
                && let Some(base_value) = base.field_value(field)
            {
                updated = updated.with_field(field, base_value);
            }
        }
        updated
    }

    pub(crate) fn tweening(&self, fields: Vec<Field>, last: i64) -> Option<Tweening> {
        if last < 1 {
            return None;
        }
        let easing = fields
            .iter()
            .filter_map(|field| self.tracks.get(field))
            .filter(|track| !track.is_empty())
            .try_fold(None, |easing: Option<Easing>, track| {
                let preset = track.tween_preset(last)?;
                Some(easing.or(Some(preset.easing)))
            })??;
        Some(Tweening {
            fields,
            last,
            easing,
        })
    }

    pub(crate) fn take_all(&mut self, fields: &[Field]) -> Vec<(Field, Tracked)> {
        fields
            .iter()
            .filter_map(|field| Some((*field, self.take(*field)?)))
            .collect()
    }

    pub(crate) fn cropped(&self, span: &Range<i64>) -> Motion {
        Motion {
            tracks: self
                .tracks
                .iter()
                .map(|(field, track)| (*field, track.cropped(span)))
                .filter(|(_, track)| !track.is_empty())
                .collect(),
        }
    }
}

fn changed<T: Animated>(current: &T, target: &T, field: Field) -> Option<(Value, Value)> {
    let (before, after) = (current.field_value(field)?, target.field_value(field)?);
    after.differs(before).then_some((before, after))
}

fn landed(field: Field, track: &Tracked, at: i64, before: Value, after: Value) -> Value {
    match (field.is_angle(), track.sample(at as f64)) {
        (true, Some(sampled)) => shifted(field, sampled, before, after),
        _ => after,
    }
}

fn shifted(field: Field, value: Value, before: Value, after: Value) -> Value {
    match (value, before, after) {
        (Value::Scalar(value), Value::Scalar(before), Value::Scalar(after)) => {
            let by = match field.is_angle() {
                true => turned(after - before),
                false => after - before,
            };
            Value::Scalar(value + by)
        }
        (Value::Point(value), Value::Point(before), Value::Point(after)) => {
            Value::Point(value + (after - before))
        }
        _ => after,
    }
}

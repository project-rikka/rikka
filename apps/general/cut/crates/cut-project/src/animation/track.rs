use rikka_keyframe::{Around, Sequence};
use rikka_types::Point;

use super::key::{Handle, Incoming, Key, Outgoing, Smooth};
use super::kind::{Kind, finite};
use super::leg::Leg;
use super::route::Route;

const THIRD: f32 = 1.0 / 3.0;
const NOWHERE: f64 = 1e-9;

#[derive(Clone, Debug, Default)]
pub struct Track<K: Kind> {
    keys: Sequence<Key<K>>,
    legs: Vec<Leg>,
    bases: Vec<f64>,
}

impl<K: Kind> PartialEq for Track<K> {
    fn eq(&self, other: &Track<K>) -> bool {
        self.keys() == other.keys()
    }
}

impl<K: Kind> Track<K> {
    #[must_use]
    pub fn new(keys: impl IntoIterator<Item = Key<K>>) -> Track<K> {
        let mut track = Track::empty();
        for key in keys {
            track.insert(key);
        }
        track.settle();
        track
    }

    #[must_use]
    pub fn keys(&self) -> &[Key<K>] {
        self.keys.keys()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    #[must_use]
    pub fn key(&self, at: i64) -> Option<&Key<K>> {
        self.keys.key(at)
    }

    #[must_use]
    pub fn previous(&self, before: i64) -> Option<&Key<K>> {
        self.keys.previous(before)
    }

    #[must_use]
    pub fn next(&self, after: i64) -> Option<&Key<K>> {
        self.keys.next(after)
    }

    #[must_use]
    pub fn sample(&self, at: f64) -> Option<K::Value> {
        Some(match self.keys.around(at)? {
            Around::Held(key) => key.value,
            Around::Between(segment) => {
                self.legs[segment.index].value(segment.from, segment.to, segment.offset)
            }
        })
    }

    #[must_use]
    pub fn speed(&self, at: f64) -> Option<f64> {
        let (first, last) = (self.keys.first()?, self.keys.last()?);
        if at < first.at as f64 || at > last.at as f64 {
            return Some(0.0);
        }
        let Some(index) = self.keys.segment_at(at) else {
            return Some(0.0);
        };
        Some(self.legs[index].speed(at - self.keys()[index].at as f64))
    }

    pub fn set(&mut self, at: i64, value: K::Value) {
        if !finite::<K>(value) {
            return;
        }
        let key = match self.keys.key(at) {
            Some(key) => Key { value, ..*key },
            None => Key::new(at, value),
        };
        self.keys.insert(key);
        self.settle();
    }

    pub fn put(&mut self, key: Key<K>) {
        self.insert(key);
        self.settle();
    }

    pub fn remove(&mut self, at: i64) -> Option<Key<K>> {
        let removed = self.keys.remove(at)?;
        self.settle();
        Some(removed)
    }

    pub(crate) fn change(&mut self, at: i64, change: impl FnOnce(&mut Key<K>)) -> bool {
        let Some(mut key) = self.keys.remove(at) else {
            return false;
        };
        change(&mut key);
        self.put(Key { at, ..key });
        true
    }

    pub(crate) fn map_values(&mut self, change: impl Fn(K::Value) -> K::Value) {
        self.keys.edit(|keys| {
            for key in keys {
                key.value = change(key.value);
            }
        });
        self.settle();
    }

    pub(crate) fn take(&mut self, frames: &[i64]) -> Vec<Key<K>> {
        let cut = self.keys.edit(|keys| {
            let (cut, remaining) = std::mem::take(keys)
                .into_iter()
                .partition(|key| frames.contains(&key.at));
            *keys = remaining;
            cut
        });
        self.settle();
        cut
    }

    pub(crate) fn from_normalized(keys: Vec<Key<K>>) -> Track<K> {
        let mut track = Track {
            keys: keys.into_iter().collect(),
            ..Track::empty()
        };
        track.settle();
        track
    }

    pub(crate) fn leg(&self, index: usize) -> Leg {
        self.legs[index]
    }

    pub(crate) fn bases(&self) -> &[f64] {
        &self.bases
    }

    fn empty() -> Track<K> {
        Track {
            keys: Sequence::new(),
            legs: Vec::new(),
            bases: Vec::new(),
        }
    }

    pub(crate) fn leg_at(&self, at: f64) -> usize {
        self.keys()
            .partition_point(|key| key.at as f64 <= at)
            .saturating_sub(1)
    }

    pub(crate) fn find(&self, at: i64) -> Result<usize, usize> {
        self.keys.find(at)
    }

    fn insert(&mut self, key: Key<K>) {
        let bent = K::bend(&key.bend).is_none_or(|bend| bend.is_finite());
        let Some(timing) = key.timing.checked() else {
            return;
        };
        if !finite::<K>(key.value) || !bent {
            return;
        }
        let mut key = Key { timing, ..key };
        if let Some(bend) = K::bend_mut(&mut key.bend) {
            *bend = bend.continued();
        }
        self.keys.insert(key);
    }

    fn settle(&mut self) {
        self.normalize_bends();
        self.normalize_speeds();
        self.legs = self
            .keys()
            .windows(2)
            .map(|pair| Leg::between(&pair[0], &pair[1]))
            .collect();
        self.bases = match K::DIMENSIONS {
            1 => self
                .keys()
                .iter()
                .map(|key| K::coords(key.value)[0])
                .collect(),
            _ => std::iter::once(0.0)
                .chain(self.legs.iter().scan(0.0, |total, leg| {
                    *total += leg.route.length();
                    Some(*total)
                }))
                .collect(),
        };
    }

    fn normalize_bends(&mut self) {
        let points: Vec<[f64; 2]> = self
            .keys()
            .iter()
            .map(|key| {
                let coords = K::coords(key.value);
                [coords[0], coords[1]]
            })
            .collect();
        self.keys.edit(|keys| {
            for (index, key) in keys.iter_mut().enumerate() {
                let Some(bend) = K::bend_mut(&mut key.bend) else {
                    return;
                };
                if bend.smooth != Smooth::Auto {
                    continue;
                }
                let before = index.checked_sub(1).map(|before| points[before]);
                let after = points.get(index + 1).copied();
                (bend.incoming, bend.outgoing) = auto_tangents(before, points[index], after);
            }
        });
    }

    fn normalize_speeds(&mut self) {
        if !self
            .keys()
            .iter()
            .any(|key| key.timing.smooth == Smooth::Auto)
        {
            return;
        }
        let legs: Vec<(f64, f64)> = self
            .keys()
            .windows(2)
            .map(|pair| {
                let travel = Route::between(&pair[0], &pair[1]).length();
                (travel, (pair[1].at - pair[0].at) as f64)
            })
            .collect();
        self.keys.edit(|keys| {
            for (index, key) in keys.iter_mut().enumerate() {
                if key.timing.smooth != Smooth::Auto {
                    continue;
                }
                let before = index.checked_sub(1).map(|before| legs[before]);
                let after = legs.get(index).copied();
                let speed = match (before, after) {
                    (Some(before), Some(after)) => (before.0 + after.0) / (before.1 + after.1),
                    (Some(only), None) | (None, Some(only)) => only.0 / only.1,
                    (None, None) => 0.0,
                };
                let handle = Handle::new(speed as f32, THIRD);
                if let Incoming::Bezier(_) = key.timing.incoming {
                    key.timing.incoming = Incoming::Bezier(handle);
                }
                if let Outgoing::Bezier(_) = key.timing.outgoing {
                    key.timing.outgoing = Outgoing::Bezier(handle);
                }
            }
        });
    }
}

fn auto_tangents(
    before: Option<[f64; 2]>,
    here: [f64; 2],
    after: Option<[f64; 2]>,
) -> (Point, Point) {
    let point = |[x, y]: [f64; 2]| Point::new(x as f32, y as f32);
    let toward =
        |to: [f64; 2], scale: f64| point([(to[0] - here[0]) * scale, (to[1] - here[1]) * scale]);
    match (before, after) {
        (Some(before), Some(after)) => {
            let across = [after[0] - before[0], after[1] - before[1]];
            let length = across[0].hypot(across[1]);
            if length < NOWHERE {
                return (Point::ZERO, Point::ZERO);
            }
            let reach = |to: [f64; 2]| (to[0] - here[0]).hypot(to[1] - here[1]) / 3.0 / length;
            let (back, ahead) = (reach(before), reach(after));
            (
                point([-across[0] * back, -across[1] * back]),
                point([across[0] * ahead, across[1] * ahead]),
            )
        }
        (None, Some(after)) => {
            let outgoing = toward(after, 1.0 / 3.0);
            (outgoing * -1.0, outgoing)
        }
        (Some(before), None) => {
            let incoming = toward(before, 1.0 / 3.0);
            (incoming, incoming * -1.0)
        }
        (None, None) => (Point::ZERO, Point::ZERO),
    }
}

use std::ops::RangeInclusive;

use rikka_keyframe::Frame;
use rikka_types::{Point, Rect};

use crate::scale::TimeScale;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
    pub from: Point,
    pub to: Point,
}

impl Band {
    #[must_use]
    pub fn at(point: Point) -> Band {
        Band {
            from: point,
            to: point,
        }
    }

    #[must_use]
    pub fn rect(self) -> Rect {
        Rect::new(
            self.from.x.min(self.to.x),
            self.from.y.min(self.to.y),
            (self.from.x - self.to.x).abs(),
            (self.from.y - self.to.y).abs(),
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pick<T> {
    Toggled(Vec<T>),
    Held {
        picked: Vec<T>,
        solo: Option<Vec<T>>,
    },
}

#[must_use]
pub fn press<T: Clone + PartialEq>(picked: &[T], hit: Vec<T>, adding: bool) -> Pick<T> {
    let all_held = hit.iter().all(|key| picked.contains(key));
    match (adding, all_held) {
        (true, true) => Pick::Toggled(
            picked
                .iter()
                .filter(|key| !hit.contains(key))
                .cloned()
                .collect(),
        ),
        (true, false) => {
            let mut more = picked.to_vec();
            more.extend(hit.into_iter().filter(|key| !picked.contains(key)));
            Pick::Held {
                picked: more,
                solo: None,
            }
        }
        (false, true) => Pick::Held {
            picked: picked.to_vec(),
            solo: (hit.len() < picked.len()).then_some(hit),
        },
        (false, false) => Pick::Held {
            picked: hit,
            solo: None,
        },
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct KeyShift<F> {
    at: f32,
    anchor: F,
    reach: RangeInclusive<i64>,
    by: i64,
    snapped: Option<F>,
}

impl<F: Frame> KeyShift<F> {
    #[must_use]
    pub fn new(at: f32, anchor: F) -> KeyShift<F> {
        KeyShift {
            at,
            anchor,
            reach: i64::MIN..=i64::MAX,
            by: 0,
            snapped: None,
        }
    }

    #[must_use]
    pub fn within(self, reach: RangeInclusive<i64>) -> KeyShift<F> {
        KeyShift { reach, ..self }
    }

    #[must_use]
    pub fn anchor(&self) -> F {
        self.anchor
    }

    #[must_use]
    pub fn by(&self) -> i64 {
        self.by
    }

    #[must_use]
    pub fn snapped(&self) -> Option<F> {
        self.snapped
    }

    pub fn drag(&mut self, x: f32, scale: &TimeScale, pull: impl FnOnce(F) -> Option<i64>) -> bool {
        let clamp = |by: i64| by.clamp(*self.reach.start(), *self.reach.end());
        let before = (self.by, self.snapped);
        let mut by = clamp(scale.frames_across(x - self.at).round() as i64);
        self.snapped = None;
        if let Some(pulled) = pull(self.anchor.offset(by))
            && clamp(by + pulled) == by + pulled
        {
            by += pulled;
            self.snapped = Some(self.anchor.offset(by));
        }
        self.by = by;
        (self.by, self.snapped) != before
    }
}

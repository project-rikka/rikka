use std::time::Instant;

use rikka_animation::{Curve, Motion, Property};
use rikka_compositor_desktop::{SnapshotId, WindowId};
use rikka_compositor_geometry::{Logical, Rect};
use rikka_compositor_types::OutputId;

use super::track::Track;
use crate::render::{Leaving, Visual};

const SWITCH: Curve = Motion::Emphasized.curve(Property::Spatial);

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum SlidePosition {
    Still,
    Along(f64),
    Elsewhere,
}

pub(crate) fn slide_apart(
    area: Rect<i32, Logical>,
    onward: bool,
    leaving: impl IntoIterator<Item = Rect<i32, Logical>>,
    arriving: impl IntoIterator<Item = Rect<i32, Logical>>,
) -> f32 {
    let (leaves, arrives): (Clearing, Clearing) = match onward {
        true => (past_start, past_end),
        false => (past_end, past_start),
    };
    let overlaps_area = |drawn: &Rect<i32, Logical>| drawn.overlaps(area);
    let distance = leaving
        .into_iter()
        .filter(overlaps_area)
        .map(|drawn| leaves(area, drawn))
        .chain(
            arriving
                .into_iter()
                .filter(overlaps_area)
                .map(|drawn| arrives(area, drawn)),
        )
        .fold(area.size.w, i32::max) as f32;
    match onward {
        true => distance,
        false => -distance,
    }
}

type Clearing = fn(Rect<i32, Logical>, Rect<i32, Logical>) -> i32;

fn past_start(area: Rect<i32, Logical>, drawn: Rect<i32, Logical>) -> i32 {
    drawn.loc.x + drawn.size.w - area.loc.x
}

fn past_end(area: Rect<i32, Logical>, drawn: Rect<i32, Logical>) -> i32 {
    area.loc.x + area.size.w - drawn.loc.x
}

#[derive(Debug)]
pub(crate) struct Slide {
    pub(crate) output: OutputId,
    travel: Track,
    stepped: Instant,
    apart: f32,
    pub(crate) leaving: Vec<(SnapshotId, Rect<i32, Logical>)>,
    arriving: Vec<WindowId>,
}

impl Slide {
    pub(crate) fn new(
        output: OutputId,
        apart: f32,
        leaving: Vec<(SnapshotId, Rect<i32, Logical>)>,
        arriving: Vec<WindowId>,
        now: Instant,
    ) -> Self {
        let mut travel = Track::new(apart, SWITCH);
        travel.aim(0.0, SWITCH);
        Self {
            output,
            travel,
            stepped: now,
            apart,
            leaving,
            arriving,
        }
    }

    pub(crate) fn carries(&self, window: WindowId) -> bool {
        self.arriving.contains(&window)
    }

    pub(crate) fn advance(&mut self, now: Instant) {
        let elapsed = now.saturating_duration_since(self.stepped);
        self.stepped = self.stepped.max(now);
        self.travel.step(elapsed);
    }

    pub(crate) fn is_finished(&self) -> bool {
        self.travel.is_resting()
    }

    pub(crate) fn arriving(&self) -> f64 {
        f64::from(self.travel.value())
    }

    pub(crate) fn leaving(&self) -> impl Iterator<Item = Leaving> + '_ {
        let offset = f64::from(self.travel.value() - self.apart);
        self.leaving
            .iter()
            .map(move |(snapshot, geometry)| Leaving {
                snapshot: *snapshot,
                geometry: *geometry,
                shown: Visual::opaque(*geometry).shifted(offset),
            })
    }
}

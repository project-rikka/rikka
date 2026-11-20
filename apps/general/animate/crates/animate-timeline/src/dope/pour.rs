use animate_fluid::Pour;
use animate_sheet::Pouring;
use animate_track::{Edge, Span};
use rikka_app::prelude::*;

use crate::layout::{Geometry, PourRow, Row};

const EDGE: f32 = 6.0;
const INSET: f32 = 4.0;
const MARK: f32 = 12.0;
const SNAP: f32 = 6.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct PourDrag {
    grip: Edge,
    anchor: f32,
    from: Pouring,
}

impl PourDrag {
    pub(crate) fn grip(&self) -> Edge {
        self.grip
    }

    pub(crate) fn from(&self) -> Pour {
        self.from.pour
    }

    pub(crate) fn to(&self, geometry: &Geometry, playhead: u32, x: f32) -> Pour {
        let pour = self.from.pour;
        let span = Span {
            start: pour.start(),
            end: self.from.flows.then(|| pour.end()),
        }
        .dragged(self.grip, self.anchor, geometry.frame_at(x), |frame| {
            nudge(geometry, playhead, frame)
        });
        match self.grip {
            Edge::Body => pour.moved_to(span.start),
            Edge::Start => pour.starting_at(span.start),
            Edge::End => span.end.map_or(pour, |end| pour.ending_at(end)),
        }
    }
}

pub(crate) fn nudge(geometry: &Geometry, playhead: u32, frame: i64) -> Option<i64> {
    let nudge = i64::from(playhead) - frame;
    (nudge.unsigned_abs() as f32 <= SNAP / geometry.zoom()).then_some(nudge)
}

pub(crate) fn grip_on(bar: Rect, x: f32, ends: bool) -> Option<Edge> {
    if x < bar.min_x() - EDGE / 2.0 || x > bar.max_x() + EDGE / 2.0 {
        return None;
    }
    if !ends {
        return Some(Edge::Body);
    }
    let edge = EDGE.min(bar.width() / 3.0);
    Some(match x {
        x if x <= bar.min_x() + edge => Edge::Start,
        x if x >= bar.max_x() - edge => Edge::End,
        _ => Edge::Body,
    })
}

pub(crate) fn band(geometry: &Geometry, offset: f32) -> Rect {
    let keys = geometry.keys();
    Rect::new(
        keys.x(),
        geometry.top_at(offset),
        keys.width(),
        Row::Pour.height(),
    )
}

pub(crate) fn bar(geometry: &Geometry, band: Rect, pouring: Pouring) -> Rect {
    let start = geometry.x_of(pouring.pour.start() as f32);
    let (x, width) = match pouring.flows {
        true => (
            start,
            (geometry.x_of(pouring.pour.end() as f32) - start).max(1.0),
        ),
        false => (start - MARK / 2.0, MARK),
    };
    Rect::new(x, band.y() + INSET, width, band.height() - 2.0 * INSET)
}

pub(crate) fn grip_at(geometry: &Geometry, row: PourRow, point: Point) -> Option<Edge> {
    let band = band(geometry, row.offset);
    if !band.contains(point) || !geometry.keys().contains(point) {
        return None;
    }
    grip_on(bar(geometry, band, row.pouring), point.x, row.pouring.flows)
}

pub(crate) fn grab(geometry: &Geometry, row: PourRow, point: Point) -> Option<PourDrag> {
    Some(PourDrag {
        grip: grip_at(geometry, row, point)?,
        anchor: geometry.frame_at(point.x),
        from: row.pouring,
    })
}

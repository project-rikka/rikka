use std::borrow::Cow;

use rikka_types::{Point, ScrollDelta, Stylus};
use smallvec::SmallVec;

use crate::input::Event;
use crate::runtime::layout::{Layouts, Node};
use crate::runtime::table::{Table, ZOOMED};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Zoom {
    factor: f32,
    pivot: Point,
}

impl Zoom {
    pub(crate) fn of(table: &Table, node: Node) -> Option<Zoom> {
        if !table.has(node, ZOOMED) {
            return None;
        }
        table
            .get(node)
            .outputs
            .subtree_scale
            .filter(|(factor, _)| *factor > 0.0 && *factor != 1.0)
            .map(|(factor, pivot)| Zoom { factor, pivot })
    }

    pub(crate) fn unzoom(self, point: Point) -> Point {
        self.pivot + (point - self.pivot) * (1.0 / self.factor)
    }

    fn unzoom_delta(self, delta: Point) -> Point {
        delta * (1.0 / self.factor)
    }

    fn unzoom_scroll(self, delta: ScrollDelta) -> ScrollDelta {
        delta.scaled(1.0 / self.factor)
    }
}

pub(crate) struct Delivered<'a> {
    pub(crate) event: Cow<'a, Event>,
    pub(crate) stylus: Option<Cow<'a, Stylus>>,
}

pub(crate) fn into_subtree<'a>(
    table: &Table,
    layouts: &Layouts,
    node: Node,
    event: &'a Event,
    stylus: Option<&'a Stylus>,
) -> Delivered<'a> {
    let borrowed = Delivered {
        event: Cow::Borrowed(event),
        stylus: stylus.map(Cow::Borrowed),
    };
    if !is_placed(event) {
        return borrowed;
    }
    let mut zooms: SmallVec<[Zoom; 4]> = SmallVec::new();
    let mut above = layouts.parent(node);
    while let Some(ancestor) = above {
        zooms.extend(Zoom::of(table, ancestor));
        above = layouts.parent(ancestor);
    }
    if zooms.is_empty() {
        return borrowed;
    }
    let point = |point: Point| zooms.iter().rev().fold(point, |at, zoom| zoom.unzoom(at));
    let delta = |delta: Point| {
        zooms
            .iter()
            .rev()
            .fold(delta, |by, zoom| zoom.unzoom_delta(by))
    };
    let stylus = stylus.map(|stylus| {
        let mut mapped = stylus.clone();
        mapped.map_positions(point);
        Cow::Owned(mapped)
    });
    let mut mapped = event.clone();
    match &mut mapped {
        Event::PointerMoved(position)
        | Event::PointerPressed { position, .. }
        | Event::PointerReleased { position, .. }
        | Event::Gesture { position, .. }
        | Event::DragMoved { position, .. }
        | Event::DragDropped { position, .. } => *position = point(*position),
        Event::PointerTravelled(by) => *by = delta(*by),
        Event::Scroll {
            position,
            delta: by,
            ..
        } => {
            *position = point(*position);
            *by = zooms
                .iter()
                .rev()
                .fold(*by, |by, zoom| zoom.unzoom_scroll(by));
        }
        _ => {}
    }
    Delivered {
        event: Cow::Owned(mapped),
        stylus,
    }
}

fn is_placed(event: &Event) -> bool {
    matches!(
        event,
        Event::PointerMoved(_)
            | Event::PointerTravelled(_)
            | Event::PointerPressed { .. }
            | Event::PointerReleased { .. }
            | Event::Scroll { .. }
            | Event::Gesture { .. }
            | Event::DragMoved { .. }
            | Event::DragDropped { .. }
    )
}

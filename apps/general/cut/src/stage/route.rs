use cut_project::{
    Bend, Channel, ClipId, Edit, KeyChange, PlacementField, Project, Projection, Property, Side,
    Smooth, Spatial, Track, Value,
};
use cut_render::Overrides;
use rikka_app::prelude::*;
use rikka_figure::GRIP_REACH;
use rikka_figure_editing::paint_line;
use rikka_ui::DRAG_SLOP;

use super::Stage;
use super::items::Item;
use super::viewing::Viewing;

const POSITION: Property = Property::Placement(PlacementField::Position);
const MAX_DOTS: usize = 2000;
const PIECES: usize = 24;
const MIN_TANGENT: f32 = 0.5;
const KEY: f32 = 8.0;
const HANDLE: f32 = 7.0;
const DOT: f32 = 3.0;
const LINE: f32 = 1.0;
const DOT_ALPHA: f32 = 0.8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Grab {
    Key,
    Handle(Side),
}

#[derive(Clone, Copy, Debug)]
struct Stop {
    at: i64,
    value: Point,
    bend: Bend,
    incoming: bool,
    outgoing: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct Route {
    clip: ClipId,
    path: Projection,
    segments: Vec<[Point; 4]>,
    stops: Vec<Stop>,
    track: Track<Spatial>,
}

#[derive(Clone, Debug)]
pub(crate) struct HandleDrag {
    clip: ClipId,
    at: i64,
    grab: Grab,
    value: Point,
    bend: Bend,
    path: Projection,
    offset: Point,
    origin: Point,
    toggles: bool,
    moved: bool,
    edit: Option<Edit>,
}

impl Stop {
    fn handle(&self, side: Side) -> Option<Point> {
        let (incoming, outgoing) = self.bend.tangents();
        let (visible, tangent) = match side {
            Side::Incoming => (self.incoming, incoming),
            Side::Outgoing => (self.outgoing, outgoing),
        };
        (visible && tangent.x.hypot(tangent.y) >= MIN_TANGENT).then(|| self.value + tangent)
    }
}

impl Stage {
    pub(super) fn route(&self, item: &Item, overrides: &Overrides) -> Option<Route> {
        let project = self.animated_preview(item.clip, overrides)?;
        Route::for_item(item, &project)
    }
}

impl Route {
    pub(super) fn for_item(item: &Item, project: &Project) -> Option<Route> {
        let (_, clip) = project.locate(item.clip)?;
        let Some(Channel::Spatial(track)) = clip.channel(&POSITION) else {
            return None;
        };
        let keys = track.keys();
        let stops = keys
            .iter()
            .enumerate()
            .map(|(index, key)| Stop {
                at: clip.start() + key.at,
                value: key.value,
                bend: key.bend,
                incoming: index > 0,
                outgoing: index + 1 < keys.len(),
            })
            .collect();
        Some(Route {
            clip: item.clip,
            path: item.projection.as_ref()?.path().clone(),
            segments: track.path(),
            stops,
            track: track.clone(),
        })
    }

    fn dots(&self) -> impl Iterator<Item = Point> + '_ {
        let keys = self.track.keys();
        let (first, last) = match (keys.first(), keys.last()) {
            (Some(first), Some(last)) => (first.at, last.at),
            _ => (0, -1),
        };
        let step = (usize::try_from(last - first).unwrap_or(0) / MAX_DOTS).max(1);
        (first..=last)
            .step_by(step)
            .filter_map(|at| self.track.sample(at as f64))
    }

    fn to_screen(&self, viewing: &Viewing, value: Point) -> Option<Point> {
        self.path.map(value).map(|at| viewing.to_screen(at))
    }

    pub(crate) fn spot_at(
        &self,
        position: Point,
        viewing: &Viewing,
        hidden: Option<i64>,
    ) -> Option<(usize, Grab)> {
        let near = |value: Point| {
            self.to_screen(viewing, value)
                .is_some_and(|spot| spot.distance(position) <= GRIP_REACH)
        };
        let handles = self.stops.iter().enumerate().flat_map(|(index, stop)| {
            [Side::Incoming, Side::Outgoing]
                .into_iter()
                .filter(move |side| stop.handle(*side).is_some_and(near))
                .map(move |side| (index, Grab::Handle(side)))
        });
        let keys = self
            .stops
            .iter()
            .enumerate()
            .filter(|(_, stop)| Some(stop.at) != hidden && near(stop.value))
            .map(|(index, _)| (index, Grab::Key));
        handles.chain(keys).next()
    }

    pub(crate) fn steer(
        &self,
        (index, grab): (usize, Grab),
        at: Point,
        origin: Point,
        toggles: bool,
    ) -> Option<HandleDrag> {
        let stop = self.stops.get(index)?;
        let grabbed = match grab {
            Grab::Key => stop.value,
            Grab::Handle(side) => stop.handle(side)?,
        };
        let pointer = self.path.unmap(at)?;
        Some(HandleDrag {
            clip: self.clip,
            at: stop.at,
            grab,
            value: stop.value,
            bend: stop.bend,
            path: self.path.clone(),
            offset: grabbed - pointer,
            origin,
            toggles,
            moved: false,
            edit: None,
        })
    }

    pub(crate) fn paint(&self, ctx: &mut PaintCtx<'_>, viewing: &Viewing, playhead: i64) {
        let accent = palette().accent;
        for segment in &self.segments {
            let points: Option<Vec<Point>> = (0..=PIECES)
                .map(|piece| self.to_screen(viewing, along(segment, piece as f32 / PIECES as f32)))
                .collect();
            for pair in points.unwrap_or_default().windows(2) {
                paint_line(ctx, pair[0], pair[1], accent);
            }
        }
        for dot in self.dots() {
            if let Some(spot) = self.to_screen(viewing, dot) {
                ctx.draw_circle(spot, DOT, accent.scale_alpha(DOT_ALPHA));
            }
        }
        for stop in &self.stops {
            let Some(key) = self.to_screen(viewing, stop.value) else {
                continue;
            };
            for side in [Side::Incoming, Side::Outgoing] {
                if let Some(handle) = stop.handle(side).and_then(|at| self.to_screen(viewing, at)) {
                    paint_line(ctx, key, handle, accent);
                    ctx.draw_circle(handle, HANDLE, accent);
                    ctx.draw_circle(handle, HANDLE - LINE * 2.0, palette().surface);
                }
            }
            let fill = match stop.at == playhead {
                true => accent,
                false => palette().surface,
            };
            ctx.draw_quad(
                Quad::new(Rect::from_center(key, Size::new(KEY, KEY)))
                    .fill(fill)
                    .stroke(LINE, accent),
            );
        }
    }
}

impl HandleDrag {
    pub(crate) fn has_moved(&self) -> bool {
        self.moved
    }

    pub(crate) fn drag(&mut self, at: Point, position: Point, breaks: bool) -> Option<Edit> {
        if !self.moved && self.origin.distance(position) < DRAG_SLOP {
            return None;
        }
        self.moved = true;
        let reached = self.path.unmap(at)? + self.offset;
        let change = match self.grab {
            Grab::Key => KeyChange::Value {
                at: self.at,
                value: Value::Point(reached),
            },
            Grab::Handle(side) => {
                let bend = match breaks {
                    true => Bend {
                        smooth: Smooth::Manual,
                        ..self.bend
                    },
                    false => self.bend,
                };
                KeyChange::tangent(self.at, bend, side, reached - self.value)
            }
        };
        let edit = self.edit_of(change);
        self.edit = Some(edit.clone());
        Some(edit)
    }

    pub(crate) fn finish(self) -> Option<Edit> {
        if self.moved {
            return self.edit;
        }
        if !self.toggles || self.grab != Grab::Key {
            return None;
        }
        let bend = match self.bend.straight {
            true => Bend::default(),
            false => Bend::STRAIGHT,
        };
        Some(self.edit_of(KeyChange::Bend { at: self.at, bend }))
    }

    fn edit_of(&self, change: KeyChange) -> Edit {
        Edit::Key {
            clip: self.clip,
            property: POSITION,
            change,
        }
    }
}

fn along(segment: &[Point; 4], t: f32) -> Point {
    let lerp = |from: Point, to: Point| from + (to - from) * t;
    let [a, b, c, d] = *segment;
    let (ab, bc, cd) = (lerp(a, b), lerp(b, c), lerp(c, d));
    let (abc, bcd) = (lerp(ab, bc), lerp(bc, cd));
    lerp(abc, bcd)
}

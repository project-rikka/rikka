use rikka_types::{Point, PointerButton, Rect, ScrollDelta, Size};

use crate::input::{Event, EventCtx, ScrollSource};
use crate::motion;
use crate::paint::{PaintCtx, Quad};
use crate::scroll::place::{Place, ScrollPosition};
use crate::style::Axis;

pub const WIDTH: f32 = 6.0;
const MARGIN: f32 = 2.0;
const MIN_THUMB: f32 = 24.0;

const IDLE_WIDTH: f32 = 0.55;

const IDLE_ALPHA: f32 = 0.2;
const HOVER_ALPHA: f32 = 0.35;
const HELD_ALPHA: f32 = 0.5;

pub const FLOOR: f32 = 1.0;
const STRAYED: f32 = 1.0;
const RETURNED: f32 = 0.5;

pub const LANE: f32 = WIDTH + MARGIN * 2.0;

fn band(axis: Axis, bounds: Rect, start: f32, extent: f32, thickness: f32) -> Rect {
    match axis {
        Axis::Vertical => Rect::new(bounds.max_x() - WIDTH - MARGIN, start, thickness, extent),
        Axis::Horizontal => Rect::new(start, bounds.max_y() - WIDTH - MARGIN, extent, thickness),
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Lanes {
    pub down: bool,
    pub across: bool,
}

impl Lanes {
    pub fn needed(room: Rect, content: Size) -> Self {
        Self {
            down: furthest(room.height(), content.height) > 0.0,
            across: furthest(room.width(), content.width) > 0.0,
        }
    }

    pub fn room(self, bounds: Rect, reserved: Size) -> Rect {
        Rect::new(
            bounds.x(),
            bounds.y(),
            (bounds.width() - if self.down { lane(reserved.width) } else { 0.0 }).max(0.0),
            (bounds.height()
                - if self.across {
                    lane(reserved.height)
                } else {
                    0.0
                })
            .max(0.0),
        )
    }
}

pub fn lane(reserved: f32) -> f32 {
    (LANE - reserved).max(0.0)
}

pub fn track(axis: Axis, bounds: Rect, room: Rect) -> Rect {
    match axis {
        Axis::Vertical => Rect::new(bounds.x(), room.y(), bounds.width(), room.height()),
        Axis::Horizontal => Rect::new(room.x(), bounds.y(), room.width(), bounds.height()),
    }
}

pub fn max_scroll(axis: Axis, bounds: Rect, content: f32) -> f32 {
    furthest(axis.room(bounds), content)
}

pub fn furthest(room: f32, content: f32) -> f32 {
    let overflow = content - room;
    if overflow < FLOOR { 0.0 } else { overflow }
}

pub fn at_end(target: f32, furthest: f32) -> bool {
    target >= furthest - FLOOR
}

pub fn away(was: bool, target: f32, furthest: f32, room: f32) -> bool {
    let behind = furthest - target;
    match was {
        true => behind > room * RETURNED,
        false => behind > room * STRAYED,
    }
}

pub fn travel(axis: Axis, delta: ScrollDelta, source: ScrollSource) -> f32 {
    let along = axis.coord(delta.offset());
    let across = axis.across().coord(delta.offset());
    match () {
        _ if along.abs() >= across.abs() => along,
        _ if axis == Axis::Horizontal && source == ScrollSource::Wheel => across,
        _ => 0.0,
    }
}

pub fn bar(axis: Axis, bounds: Rect, content: f32, offset: f32) -> Option<(Rect, Rect)> {
    let furthest = max_scroll(axis, bounds, content);
    if furthest <= 0.0 {
        return None;
    }
    let room = axis.room(bounds);
    let start = axis.coord(bounds.origin) + MARGIN;
    let track = band(axis, bounds, start, room - MARGIN * 2.0, WIDTH);
    let thumb_extent = (axis.room(track) * (room / content)).max(MIN_THUMB);
    let travelled = (offset / furthest).clamp(0.0, 1.0);
    let thumb = band(
        axis,
        bounds,
        start + travelled * (axis.room(track) - thumb_extent),
        thumb_extent,
        WIDTH,
    );
    Some((track, thumb))
}

pub fn dragged_to(
    axis: Axis,
    bounds: Rect,
    content: f32,
    offset: f32,
    grab: f32,
    pointer: Point,
) -> Option<f32> {
    let (track, thumb) = bar(axis, bounds, content, offset)?;
    let travel = (axis.room(track) - axis.room(thumb)).max(f32::EPSILON);
    let reached = axis.coord(pointer) - grab - axis.coord(track.origin);
    Some((reached / travel).clamp(0.0, 1.0) * max_scroll(axis, bounds, content))
}

pub fn reveal(offset: f32, start: f32, extent: f32, room: f32) -> f32 {
    let end = start + extent - room;
    offset.clamp(start.min(end).max(0.0), start.max(end).max(0.0))
}

pub fn follow(offset: f32, start: f32, extent: f32, room: f32, furthest: f32) -> f32 {
    reveal(offset, start, extent, room).clamp(0.0, furthest)
}

pub fn grab(axis: Axis, bounds: Rect, content: f32, offset: f32, at: Point) -> Option<f32> {
    let (track, thumb) = bar(axis, bounds, content, offset)?;
    match (thumb.contains(at), track.contains(at)) {
        (true, _) => Some(axis.coord(at) - axis.coord(thumb.origin)),
        (false, true) => Some(axis.room(thumb) / 2.0),
        (false, false) => None,
    }
}

#[derive(Debug)]
pub enum Grabbed {
    Missed,
    Moved(f32),
    Rested,
}

#[derive(Debug, Default)]
pub struct Grip {
    grab: Option<f32>,
}

impl Grip {
    pub fn is_dragging(&self) -> bool {
        self.grab.is_some()
    }

    pub fn handle_event(
        &mut self,
        axis: Axis,
        track: Rect,
        content: f32,
        offset: f32,
        event: &Event,
        ctx: &mut EventCtx<'_>,
    ) -> Grabbed {
        let taken = match *event {
            Event::PointerPressed {
                button: PointerButton::Primary,
                position,
                ..
            } if self.grab.is_none() => {
                let Some(grab) = grab(axis, track, content, offset, position) else {
                    return Grabbed::Missed;
                };
                self.grab = Some(grab);
                ctx.capture_pointer();
                Some(position)
            }
            Event::PointerMoved(position) if self.grab.is_some() => Some(position),
            Event::PointerReleased { .. } if self.grab.is_some() => {
                self.grab = None;
                ctx.release_pointer();
                None
            }
            _ => return Grabbed::Missed,
        };

        ctx.request_paint();
        ctx.set_handled();
        let reached = taken.and_then(|position| {
            dragged_to(
                axis,
                track,
                content,
                offset,
                self.grab.unwrap_or_default(),
                position,
            )
        });
        match reached {
            Some(to) => Grabbed::Moved(to),
            None => Grabbed::Rested,
        }
    }
}

#[derive(Debug, Default)]
pub struct Scroll {
    target: f32,
    rendered: f32,
    shifted: f32,
    impulse: f32,
    jumped: bool,
    mirrored: ScrollPosition,
}

impl Scroll {
    pub fn target(&self) -> f32 {
        self.target
    }

    pub fn rendered(&self) -> f32 {
        self.rendered
    }

    pub fn aim(&mut self, offset: f32) {
        if offset != self.target {
            self.impulse = 0.0;
        }
        self.target = offset;
    }

    pub fn snap(&mut self, offset: f32) {
        self.target = offset;
        self.rendered = offset;
        self.shifted = 0.0;
        self.impulse = 0.0;
        self.jumped = true;
    }

    pub fn adopt(&mut self, place: Option<&Place>, subject: Option<u64>) -> Option<ScrollPosition> {
        let stand = place?.get();
        if stand == self.mirrored || stand.subject != subject {
            return None;
        }
        self.mirrored = stand;
        self.snap(stand.offset);
        Some(stand)
    }
    pub fn heed(&mut self, place: Option<&Place>, furthest: f32, room: f32) -> bool {
        if !place.is_some_and(Place::take_end) {
            return false;
        }
        let near = (furthest - room).max(0.0);
        if self.target < near {
            self.snap(near);
        }
        self.aim(furthest);
        true
    }

    pub fn stand(&self, furthest: f32, room: f32) -> ScrollPosition {
        ScrollPosition {
            subject: None,
            offset: self.target,
            ended: at_end(self.target, furthest),
            away: away(self.mirrored.away, self.target, furthest, room),
            row: None,
        }
    }

    pub fn publish(&mut self, place: Option<&Place>, stand: ScrollPosition) {
        let Some(place) = place else {
            return;
        };
        self.mirrored = stand;
        place.record(stand);
    }

    pub fn shift(&mut self, delta: f32) {
        self.target += delta;
        self.rendered += delta;
        self.shifted += delta;
    }

    pub fn nudge(&mut self, travel: f32, furthest: f32) -> bool {
        let from = self.target.clamp(0.0, furthest);
        let next = (self.target + travel).clamp(0.0, furthest);
        self.impulse += next - from;
        self.target = next;
        next != from
    }

    pub fn advance(&mut self, ctx: &mut PaintCtx<'_>, axis: Axis, furthest: f32) -> f32 {
        let key = ("scroll", lane_name(axis));
        self.target = self.target.clamp(0.0, furthest);
        let shifted = std::mem::take(&mut self.shifted);
        match std::mem::take(&mut self.jumped) {
            true => ctx.jump(key, self.rendered),
            false => ctx.shift(key, shifted),
        }
        let impulse = std::mem::take(&mut self.impulse);
        if impulse != 0.0 {
            ctx.impel(key, motion::SCROLL.launch_speed(impulse));
        }
        self.rendered = ctx
            .animate_curve(key, self.target, motion::SCROLL)
            .clamp(0.0, furthest);
        self.rendered
    }
}

fn lane_name(axis: Axis) -> &'static str {
    match axis {
        Axis::Vertical => "down",
        Axis::Horizontal => "across",
    }
}

pub fn paint_thumb(
    ctx: &mut PaintCtx<'_>,
    axis: Axis,
    bounds: Rect,
    content: f32,
    offset: f32,
    (attentive, pressed): (bool, bool),
) {
    let Some((_, thumb)) = bar(axis, bounds, content, offset) else {
        return;
    };
    let alpha = match () {
        _ if pressed => HELD_ALPHA,
        _ if attentive => HOVER_ALPHA,
        _ => IDLE_ALPHA,
    };
    let target = ctx.theme().palette().overlay.with_alpha(alpha);
    let lane = lane_name(axis);
    let fill = ctx.animate_curve(("scrollbar", lane, "color"), target, motion::HOVER);
    let width = ctx.animate_curve(
        ("scrollbar", lane, "width"),
        if pressed || attentive {
            WIDTH
        } else {
            WIDTH * IDLE_WIDTH
        },
        motion::MORPH,
    );
    let thickness = match axis {
        Axis::Vertical => Size::new(width, thumb.height()),
        Axis::Horizontal => Size::new(thumb.width(), width),
    };
    ctx.draw_quad(
        Quad::new(Rect::from_center(thumb.center(), thickness))
            .fill(fill)
            .radius(width / 2.0),
    );
}

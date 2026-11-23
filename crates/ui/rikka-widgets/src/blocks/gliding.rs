use rikka_types::{Point, Rect, Size};
use rikka_ui::{Behavior, Element, PaintCtx, View, Widget, motion};

const NUDGE: f32 = 0.5;

const TRAVEL: &str = "travel";

#[must_use]
#[derive(Debug, Default)]
pub struct Gliding;

pub fn gliding(child: impl View) -> Element {
    Element::with_children(Gliding, vec![child.build()])
}

#[derive(Debug, Default)]
#[doc(hidden)]
pub struct GlidingState {
    seen: Option<Point>,
    shown: Point,
}

impl Widget for Gliding {
    type State = GlidingState;

    fn behavior(&self, _state: &GlidingState) -> Behavior {
        Behavior::TRAVELS | Behavior::TRACKS_PLACE
    }

    fn paint(&self, state: &mut GlidingState, ctx: &mut PaintCtx<'_>) {
        let standing = ctx.placed().origin;
        if let Some(was) = state.seen.replace(standing) {
            let travelled = Point::new(was.x - standing.x, was.y - standing.y);
            if travelled.x.abs() > NUDGE || travelled.y.abs() > NUDGE {
                match ctx.is_animating(TRAVEL) {
                    true => ctx.shift(TRAVEL, travelled),
                    false => ctx.jump(TRAVEL, travelled),
                }
            }
        }
        state.shown = ctx.animate_curve(TRAVEL, Point::ZERO, motion::GLIDE);
    }

    fn children_offset(&self, state: &GlidingState, _bounds: Rect, _content: Size) -> Point {
        state.shown
    }

    fn hit_test(&self, _bounds: Rect, _point: Point) -> bool {
        false
    }

    fn same(&self, _other: &Self) -> bool {
        true
    }
}

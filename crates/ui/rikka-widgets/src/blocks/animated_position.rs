use rikka_types::{Point, Rect, Size};
use rikka_ui::{Axis, Behavior, Element, PaintCtx, View, Widget, motion};

use crate::blocks::cell_content::{CellContent, Lazily};

const NUDGE: f32 = 0.5;

const TRAVEL: &str = "travel";

const FADE: &str = "fade";

#[must_use]
pub struct AnimatedPosition {
    along: Axis,
    at: Option<Seat>,
    arriving: bool,
    lazily: Option<Lazily>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Seat {
    index: usize,
    along: f32,
}

pub fn animated_position(child: impl View) -> Element {
    AnimatedPosition::default().holding(child)
}

impl Default for AnimatedPosition {
    fn default() -> Self {
        AnimatedPosition {
            along: Axis::Vertical,
            at: None,
            arriving: false,
            lazily: None,
        }
    }
}

impl AnimatedPosition {
    pub fn along(mut self, along: Axis) -> Self {
        self.along = along;
        self
    }

    pub fn anchored(mut self, index: usize, along: f32) -> Self {
        self.at = Some(Seat { index, along });
        self
    }

    pub fn arriving(mut self, arriving: bool) -> Self {
        self.arriving = arriving;
        self
    }

    pub fn holding(self, child: impl View) -> Element {
        self.with_content(CellContent::Built(child.build()))
    }

    pub(crate) fn with_content(mut self, content: CellContent) -> Element {
        let (children, lazily) = content.parts();
        self.lazily = lazily;
        Element::with_children(self, children)
    }

    fn standing(&self, ctx: &PaintCtx<'_>) -> Seat {
        self.at.unwrap_or(Seat {
            index: 0,
            along: self.along.coord(ctx.bounds().origin),
        })
    }

    fn travelled(&self, was: Option<Seat>, standing: Seat) -> f32 {
        let Some(was) = was else {
            return 0.0;
        };
        let moved = self.at.is_none() || was.index != standing.index;
        match moved {
            true => was.along - standing.along,
            false => 0.0,
        }
    }
}

#[derive(Debug, Default)]
#[doc(hidden)]
pub struct AnimatedPositionState {
    seen: Option<Seat>,
    shown: f32,
}

impl Widget for AnimatedPosition {
    type State = AnimatedPositionState;

    fn behavior(&self, _state: &AnimatedPositionState) -> Behavior {
        Behavior::TRAVELS | Behavior::TRACKS_PLACE.when(self.at.is_none())
    }

    fn paint(&self, state: &mut AnimatedPositionState, ctx: &mut PaintCtx<'_>) {
        let standing = self.standing(ctx);
        let travelled = self.travelled(state.seen.replace(standing), standing);
        if travelled.abs() > NUDGE {
            match ctx.is_animating(TRAVEL) {
                true => ctx.shift(TRAVEL, travelled),
                false => ctx.jump(TRAVEL, travelled),
            }
        }
        state.shown = ctx.animate_curve(TRAVEL, 0.0, motion::GLIDE);

        if !self.arriving && !ctx.is_animating(FADE) {
            ctx.jump(FADE, 1.0);
        }
        let shown = ctx.play(FADE, motion::ARRIVAL);
        if shown < 1.0 {
            ctx.set_subtree_opacity(shown);
        }
    }

    fn children_offset(
        &self,
        state: &AnimatedPositionState,
        _bounds: Rect,
        _content: Size,
    ) -> Point {
        self.along.point(state.shown, 0.0)
    }

    fn hit_test(&self, _bounds: Rect, _point: Point) -> bool {
        false
    }

    fn build(&self, _state: &mut AnimatedPositionState) -> Vec<Element> {
        self.lazily.iter().map(Lazily::build).collect()
    }

    fn memo(&self) -> Option<u64> {
        self.lazily.as_ref().map(Lazily::digest)
    }

    fn same(&self, other: &Self) -> bool {
        self.along == other.along
            && self.at == other.at
            && self.arriving == other.arriving
            && self.memo() == other.memo()
    }
}

impl std::fmt::Debug for AnimatedPosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnimatedPosition")
            .field("at", &self.at)
            .field("arriving", &self.arriving)
            .finish_non_exhaustive()
    }
}

use rikka_types::{CursorShape, Point, PointerButton, Rect};
use rikka_ui::{
    Action, Behavior, Element, Event, EventCtx, IntoHandler, Layout, Length, PaintCtx, Quad, Style,
    Styled, View, Widget, motion,
};

use crate::controls::grab::{Grab, GrabStep};
use crate::navigation::panes::Axis;
use crate::navigation::panes::split_ratios::SplitRatios;

pub const SPLIT_GUTTER: f32 = 6.0;

const RULE: f32 = 1.0;

const GRIP: f32 = 3.0;

const LEAST: f32 = 56.0;

const SHARE: f32 = 0.02;

#[must_use]
pub struct Split {
    axis: Axis,
    ratio: SplitRatios,
    first: Option<Element>,
    second: Option<Element>,
    on_resize: Action<(f32,)>,
    style: Style,
}

pub fn split(
    axis: Axis,
    ratio: impl Into<SplitRatios>,
    first: impl View,
    second: impl View,
) -> Split {
    Split {
        axis,
        ratio: ratio.into(),
        first: Some(first.build()),
        second: Some(second.build()),
        on_resize: Action::default(),
        style: Style::default(),
    }
}

impl Split {
    pub fn on_resize<Marker>(mut self, on_resize: impl IntoHandler<(f32,), (), Marker>) -> Self {
        self.on_resize = Action::new(on_resize);
        self
    }

    fn ratio(&self) -> f32 {
        clamped(&self.ratio)
    }

    fn along(&self, bounds: Rect) -> (f32, f32) {
        match self.axis {
            Axis::Horizontal => (bounds.min_x(), bounds.width()),
            Axis::Vertical => (bounds.min_y(), bounds.height()),
        }
    }

    fn gutter(&self, bounds: Rect) -> Rect {
        let (start, length) = self.along(bounds);
        let at = start + (length - SPLIT_GUTTER).max(0.0) * self.ratio();
        match self.axis {
            Axis::Horizontal => Rect::new(at, bounds.y(), SPLIT_GUTTER, bounds.height()),
            Axis::Vertical => Rect::new(bounds.x(), at, bounds.width(), SPLIT_GUTTER),
        }
    }

    fn reached(&self, point: Point) -> f32 {
        match self.axis {
            Axis::Horizontal => point.x,
            Axis::Vertical => point.y,
        }
    }

    fn ratio_at(&self, bounds: Rect, point: Point) -> f32 {
        let (start, length) = self.along(bounds);
        let room = length - SPLIT_GUTTER;
        if room <= 0.0 {
            return self.ratio();
        }
        let travelled = self.reached(point) - start - SPLIT_GUTTER / 2.0;
        let least = (LEAST / room).min(0.5);
        (travelled / room).clamp(least, 1.0 - least)
    }

    fn resize(&self, ctx: &mut EventCtx<'_>, position: Point) {
        let Some(on_resize) = self.on_resize.handler() else {
            return;
        };
        let ratio = self.ratio_at(ctx.bounds(), position);
        if ratio != self.ratio() {
            ctx.call(on_resize, (ratio,));
        }
    }

    fn movable(&self) -> bool {
        self.on_resize.is_set()
    }
}

#[derive(Debug, Default)]
#[doc(hidden)]
pub struct SplitState {
    hot: bool,
    grab: Grab,
}

impl Widget for Split {
    type State = SplitState;

    fn layout(&self) -> Layout {
        let layout = match self.axis {
            Axis::Horizontal => Layout::row().gap_exact(SPLIT_GUTTER, 0.0),
            Axis::Vertical => Layout::column().gap_exact(0.0, SPLIT_GUTTER),
        };
        layout.size(Length::FILL, Length::FILL).with(&self.style)
    }

    fn paint(&self, state: &mut SplitState, ctx: &mut PaintCtx<'_>) {
        self.ratio.watch();
        let gutter = self.gutter(ctx.bounds());
        let (outline, accent) = {
            let palette = ctx.palette();
            (palette.outline, palette.accent)
        };
        let lit = state.grab.is_held() || (state.hot && self.movable());
        let grip = ctx.fade("grip", lit.then_some(accent), motion::HOVER);
        let (thickness, fill) = match grip {
            Some(fill) => (GRIP, fill),
            None => (RULE, outline),
        };
        ctx.draw_quad(Quad::new(centred(gutter, self.axis, thickness)).fill(fill));
    }

    fn cursor(&self, _state: &Self::State, bounds: Rect, point: Point) -> Option<CursorShape> {
        if !self.movable() || !self.gutter(bounds).contains(point) {
            return None;
        }
        Some(match self.axis {
            Axis::Horizontal => CursorShape::ResizeColumn,
            Axis::Vertical => CursorShape::ResizeRow,
        })
    }

    fn event(&self, state: &mut SplitState, event: &Event, ctx: &mut EventCtx<'_>) {
        if !self.movable() {
            return;
        }
        let bounds = ctx.bounds();
        match state.grab.update(event, ctx) {
            Some(GrabStep::Moved { to, .. }) => return self.resize(ctx, to),
            Some(GrabStep::Ended { at, .. }) => return self.resize(ctx, at),
            Some(GrabStep::Cancelled { .. }) => return,
            None => {}
        }
        match event {
            Event::PointerPressed {
                button: PointerButton::Primary,
                position,
                ..
            } if self.gutter(bounds).contains(*position) => state.grab.begin((), ctx),
            Event::PointerMoved(position) => {
                let hot = self.gutter(bounds).contains(*position);
                if state.hot != hot {
                    state.hot = hot;
                    ctx.request_paint();
                }
            }
            Event::PointerExited if state.hot => {
                state.hot = false;
                ctx.request_paint();
            }
            _ => {}
        }
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::TRACKS_HOVER
    }
}

fn centred(gutter: Rect, axis: Axis, thickness: f32) -> Rect {
    match axis {
        Axis::Horizontal => Rect::new(
            gutter.center().x - thickness / 2.0,
            gutter.y(),
            thickness,
            gutter.height(),
        ),
        Axis::Vertical => Rect::new(
            gutter.x(),
            gutter.center().y - thickness / 2.0,
            gutter.width(),
            thickness,
        ),
    }
}

impl std::fmt::Debug for Split {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Split")
            .field("axis", &self.axis)
            .field("ratio", &self.ratio)
            .finish_non_exhaustive()
    }
}

impl View for Split {
    fn build(mut self) -> Element {
        let first = self.first.take().expect("first pane already taken");
        let second = self.second.take().expect("second pane already taken");
        let children = vec![
            Element::with_children(Part::new(&self.ratio, true), vec![first]),
            Element::with_children(Part::new(&self.ratio, false), vec![second]),
        ];
        Element::with_children(self, children)
    }
}

fn clamped(share: &SplitRatios) -> f32 {
    share.value().clamp(SHARE, 1.0 - SHARE)
}

struct Part {
    share: SplitRatios,
    first: bool,
}

impl Part {
    fn new(share: &SplitRatios, first: bool) -> Part {
        Part {
            share: share.clone(),
            first,
        }
    }
}

impl Widget for Part {
    type State = ();

    fn layout(&self) -> Layout {
        let ratio = clamped(&self.share);
        let grow = match self.first {
            true => ratio,
            false => 1.0 - ratio,
        };
        Layout::DEFAULT.with(&Style {
            grow: Some(grow),
            ..Style::default()
        })
    }

    fn paint(&self, (): &mut (), _ctx: &mut PaintCtx<'_>) {
        self.share.watch();
    }

    fn same(&self, other: &Self) -> bool {
        self.share == other.share && self.first == other.first
    }

    fn debug_name(&self) -> &'static str {
        "SplitPart"
    }
}

impl Styled for Split {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }
}

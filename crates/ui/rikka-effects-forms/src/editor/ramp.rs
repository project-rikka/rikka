use rikka_types::{Color, Size};
use rikka_ui::{
    Action, Behavior, Event, EventCtx, IntoHandler, Key, Layout, Length, PaintCtx, PointerButton,
    Quad, Rect, Widget,
};

use rikka_widgets::{Grab, GrabStep};

use super::notched::NotchedTrack;
use super::plot::{draw_bins, draw_ramp};

const GRAPH: f32 = 112.0;
const BAR: f32 = 10.0;
const KNOB: Size = Size::new(11.0, 12.0);
const SPACING: f32 = 3.0;
const REACH: f32 = 12.0;
const NUDGE: f32 = 1.0 / 255.0;
const LARGE_NUDGE: f32 = 10.0 / 255.0;
const GUIDE_ALPHA: f32 = 0.35;
const RAMP_SLICES: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Notch {
    pub(crate) at: f32,
    pub(crate) fill: Color,
}

impl Notch {
    pub(crate) fn new(at: f32, fill: Color) -> Notch {
        Notch {
            at: at.clamp(0.0, 1.0),
            fill,
        }
    }
}

#[derive(Widget)]
pub(crate) struct Ramp {
    #[debug]
    knobs: Vec<Notch>,
    bins: Option<Vec<f32>>,
    tint: Color,
    on_change: Action<((usize, f32),)>,
    on_commit: Action<((usize, f32),)>,
}

#[derive(Debug, Default)]
pub(crate) struct RampState {
    grab: Grab<usize>,
    active: usize,
}

impl Ramp {
    pub(crate) fn new<M, N>(
        knobs: Vec<Notch>,
        on_change: impl IntoHandler<((usize, f32),), (), M>,
        on_commit: impl IntoHandler<((usize, f32),), (), N>,
    ) -> Ramp {
        Ramp {
            knobs,
            bins: None,
            tint: Color::BLACK,
            on_change: Action::new(on_change),
            on_commit: Action::new(on_commit),
        }
    }

    pub(crate) fn histogram(mut self, bins: Option<Vec<f32>>, tint: Color) -> Ramp {
        self.bins = bins;
        self.tint = tint;
        self
    }

    fn graph_height(&self) -> f32 {
        match self.bins {
            Some(_) => GRAPH + SPACING,
            None => 0.0,
        }
    }

    fn track(&self, bounds: Rect) -> NotchedTrack {
        NotchedTrack::new(bounds, bounds.y() + self.graph_height(), BAR, KNOB)
    }

    fn nearest(&self, bounds: Rect, x: f32) -> Option<usize> {
        let ats = self.knobs.iter().map(|knob| knob.at).enumerate();
        self.track(bounds).nearest(ats, x, REACH)
    }

    fn held_at(&self, index: usize) -> f32 {
        self.knobs.get(index).map_or(0.0, |knob| knob.at)
    }
}

impl Widget for Ramp {
    type State = RampState;

    fn layout(&self) -> Layout {
        Layout::column()
            .size(
                Length::FILL,
                Length::px(self.graph_height() + BAR + SPACING + KNOB.height),
            )
            .fixed()
    }

    fn paint(&self, state: &mut RampState, ctx: &mut PaintCtx<'_>) {
        let palette = ctx.palette();
        let bounds = ctx.bounds();
        let track = self.track(bounds);
        if let Some(bins) = &self.bins {
            let graph = Rect::new(track.rect.x(), bounds.y(), track.rect.width(), GRAPH);
            ctx.draw_quad(Quad::new(graph).fill(palette.surface_variant));
            draw_bins(ctx, graph, bins, self.tint);
            for knob in &self.knobs {
                ctx.draw_quad(
                    Quad::new(Rect::new(
                        track.x(knob.at) - 0.5,
                        graph.y(),
                        1.0,
                        graph.height(),
                    ))
                    .fill(palette.text_muted.scale_alpha(GUIDE_ALPHA)),
                );
            }
        }
        draw_ramp(
            ctx,
            track.rect,
            |along| Color::rgb(along, along, along),
            RAMP_SLICES,
        );
        ctx.draw_quad(Quad::new(track.rect).stroke(1.0, palette.outline));
        let focused = ctx.focused();
        let row = track.rect.max_y() + SPACING;
        for (index, knob) in self.knobs.iter().enumerate() {
            let lit = focused && index == state.active;
            track.paint_notch(ctx, knob.at, row, knob.fill, lit);
        }
    }

    fn event(&self, state: &mut RampState, event: &Event, ctx: &mut EventCtx<'_>) {
        let bounds = ctx.bounds();
        let along = |x: f32| self.track(bounds).along(x);
        match state.grab.update(event, ctx) {
            Some(GrabStep::Moved { held, to }) => {
                return ctx.act(&self.on_change, ((held, along(to.x)),));
            }
            Some(GrabStep::Ended { held, at }) => {
                return ctx.act(&self.on_commit, ((held, along(at.x)),));
            }
            Some(GrabStep::Cancelled { held }) => {
                return ctx.act(&self.on_commit, ((held, self.held_at(held)),));
            }
            None => {}
        }
        match *event {
            Event::PointerPressed {
                button: PointerButton::Primary,
                position,
                ..
            } => {
                ctx.request_focus();
                let Some(index) = self.nearest(bounds, position.x) else {
                    return;
                };
                state.active = index;
                state.grab.begin(index, ctx);
                ctx.act(&self.on_change, ((index, along(position.x)),));
            }
            Event::KeyPressed { key, modifiers, .. } if ctx.focused() => {
                let step = match modifiers.shift {
                    true => LARGE_NUDGE,
                    false => NUDGE,
                };
                let Some(knob) = self.knobs.get(state.active) else {
                    return;
                };
                let count = self.knobs.len();
                let at = match key {
                    Key::ArrowLeft => knob.at - step,
                    Key::ArrowRight => knob.at + step,
                    Key::ArrowUp | Key::ArrowDown => {
                        state.active = match key {
                            Key::ArrowUp => (state.active + 1) % count,
                            _ => (state.active + count - 1) % count,
                        };
                        ctx.set_handled();
                        ctx.request_paint();
                        return;
                    }
                    _ => return,
                };
                ctx.set_handled();
                ctx.act(&self.on_commit, ((state.active, at.clamp(0.0, 1.0)),));
            }
            _ => {}
        }
    }

    fn same(&self, other: &Ramp) -> bool {
        self.knobs == other.knobs
            && self.bins == other.bins
            && self.tint == other.tint
            && self.on_change == other.on_change
            && self.on_commit == other.on_commit
    }

    fn behavior(&self, _state: &RampState) -> Behavior {
        Behavior::FOCUSABLE
    }
}

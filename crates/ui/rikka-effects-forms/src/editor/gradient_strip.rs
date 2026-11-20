use rikka_image_edit::{GradientMap, GradientStop};
use rikka_types::Size;
use rikka_ui::{
    Action, Behavior, Event, EventCtx, IntoHandler, Key, Layout, Length, Modifiers, PaintCtx,
    Point, PointerButton, Quad, Rect, Widget,
};

use rikka_widgets::{Grab, GrabStep};

use super::notched::NotchedTrack;
use super::plot::draw_ramp;
use rikka_effects::Gradient;

const STRIP: f32 = 26.0;
const SPACING: f32 = 3.0;
const STOP: Size = Size::new(12.0, 14.0);
const MIDPOINT: f32 = 8.0;
const REACH: f32 = 7.0;
const OUTSIDE: f32 = 24.0;
const NUDGE: f32 = 0.01;
const LARGE_NUDGE: f32 = 0.1;
const SLICES: usize = 96;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum GradientEdit {
    Select(usize),
    Insert(f32),
    Drag(f32),
    RemoveSelected,
    Midpoint(usize, f32),
}

#[derive(Widget)]
pub(crate) struct GradientStrip {
    #[debug]
    stops: Vec<GradientStop>,
    selected: usize,
    on_change: Action<(GradientEdit,)>,
    on_commit: Action<(GradientEdit,)>,
}

#[derive(Clone, Copy, Debug)]
enum Held {
    Stop,
    Midpoint(usize),
}

#[derive(Debug, Default)]
pub(crate) struct GradientStripState {
    grab: Grab<Held>,
    moved: bool,
}

impl GradientStrip {
    pub(crate) fn new<M, N>(
        gradient: &Gradient,
        selected: usize,
        on_change: impl IntoHandler<(GradientEdit,), (), M>,
        on_commit: impl IntoHandler<(GradientEdit,), (), N>,
    ) -> GradientStrip {
        GradientStrip {
            stops: gradient.stops().to_vec(),
            selected,
            on_change: Action::new(on_change),
            on_commit: Action::new(on_commit),
        }
    }

    fn track(bounds: Rect) -> NotchedTrack {
        NotchedTrack::new(bounds, bounds.y(), STRIP, STOP)
    }

    fn midpoints(&self) -> impl Iterator<Item = (usize, f32)> + '_ {
        self.stops.windows(2).enumerate().map(|(index, pair)| {
            (
                index,
                pair[0].at + (pair[1].at - pair[0].at) * pair[0].midpoint,
            )
        })
    }

    fn stop_row(bounds: Rect) -> f32 {
        bounds.y() + STRIP + SPACING
    }

    fn midpoint_row(bounds: Rect) -> f32 {
        GradientStrip::stop_row(bounds) + STOP.height + SPACING
    }

    fn pressed(&self, state: &mut GradientStripState, position: Point, ctx: &mut EventCtx<'_>) {
        let bounds = ctx.bounds();
        let track = GradientStrip::track(bounds);
        state.moved = false;
        if position.y >= GradientStrip::midpoint_row(bounds) - 1.0
            && let Some(index) = track.nearest(self.midpoints(), position.x, REACH)
        {
            state.grab.begin(Held::Midpoint(index), ctx);
            return;
        }
        let stops = self.stops.iter().map(|stop| stop.at).enumerate();
        state.grab.begin(Held::Stop, ctx);
        let edit = match track.nearest(stops, position.x, REACH) {
            Some(index) if position.y >= GradientStrip::stop_row(bounds) => {
                GradientEdit::Select(index)
            }
            _ => GradientEdit::Insert(track.along(position.x)),
        };
        ctx.act(&self.on_commit, (edit,));
    }

    fn drawn_to(&self, held: Held, position: Point, bounds: Rect) -> Option<GradientEdit> {
        let along = GradientStrip::track(bounds).along(position.x);
        match held {
            Held::Stop => Some(GradientEdit::Drag(along)),
            Held::Midpoint(index) => {
                let (from, to) = (self.stops.get(index)?, self.stops.get(index + 1)?);
                let span = (to.at - from.at).max(1e-4);
                Some(GradientEdit::Midpoint(index, (along - from.at) / span))
            }
        }
    }

    fn dragged(
        &self,
        state: &mut GradientStripState,
        held: Held,
        position: Point,
        ctx: &mut EventCtx<'_>,
    ) {
        let bounds = ctx.bounds();
        let dropped = matches!(held, Held::Stop)
            && position.y > bounds.max_y() + OUTSIDE
            && Gradient::removable(&self.stops);
        if dropped {
            state.grab.end(ctx);
            ctx.act(&self.on_commit, (GradientEdit::RemoveSelected,));
            return;
        }
        if let Some(edit) = self.drawn_to(held, position, bounds) {
            state.moved = true;
            ctx.act(&self.on_change, (edit,));
        }
    }

    fn settled(&self, held: Held) -> Option<GradientEdit> {
        match held {
            Held::Stop => Some(GradientEdit::Drag(self.stops.get(self.selected)?.at)),
            Held::Midpoint(index) => Some(GradientEdit::Midpoint(
                index,
                self.stops.get(index)?.midpoint,
            )),
        }
    }

    fn keyed(&self, key: Key, modifiers: Modifiers, ctx: &mut EventCtx<'_>) {
        let Some(stop) = self.stops.get(self.selected) else {
            return;
        };
        let step = match modifiers.shift {
            true => LARGE_NUDGE,
            false => NUDGE,
        };
        let edit = match key {
            Key::Delete if Gradient::removable(&self.stops) => GradientEdit::RemoveSelected,
            Key::ArrowLeft => GradientEdit::Drag((stop.at - step).max(0.0)),
            Key::ArrowRight => GradientEdit::Drag((stop.at + step).min(1.0)),
            Key::ArrowUp if self.selected + 1 < self.stops.len() => {
                GradientEdit::Select(self.selected + 1)
            }
            Key::ArrowDown if self.selected > 0 => GradientEdit::Select(self.selected - 1),
            _ => return,
        };
        ctx.set_handled();
        ctx.act(&self.on_commit, (edit,));
    }
}

impl Widget for GradientStrip {
    type State = GradientStripState;

    fn layout(&self) -> Layout {
        Layout::column()
            .size(
                Length::FILL,
                Length::px(STRIP + SPACING + STOP.height + SPACING + MIDPOINT),
            )
            .fixed()
    }

    fn paint(&self, _state: &mut GradientStripState, ctx: &mut PaintCtx<'_>) {
        let palette = ctx.palette();
        let bounds = ctx.bounds();
        let track = GradientStrip::track(bounds);
        let map = GradientMap::new(self.stops.iter().copied());
        draw_ramp(ctx, track.rect, |along| map.color_at(along), SLICES);
        let edge = match ctx.focused() {
            true => palette.accent,
            false => palette.outline,
        };
        ctx.draw_quad(Quad::new(track.rect).stroke(1.0, edge));
        let row = GradientStrip::stop_row(bounds);
        for (index, stop) in self.stops.iter().enumerate() {
            let x = track.x(stop.at);
            ctx.draw_quad(
                Quad::new(Rect::new(
                    x - 0.5,
                    track.rect.max_y(),
                    1.0,
                    row - track.rect.max_y(),
                ))
                .fill(palette.text_muted),
            );
            track.paint_notch(ctx, stop.at, row, stop.color, index == self.selected);
        }
        let middle = GradientStrip::midpoint_row(bounds) + MIDPOINT / 2.0;
        for (index, at) in self.midpoints() {
            let near = index == self.selected || index + 1 == self.selected;
            let color = match near {
                true => palette.accent,
                false => palette.text_muted,
            };
            ctx.draw_circle(Point::new(track.x(at), middle), MIDPOINT, color);
        }
    }

    fn event(&self, state: &mut GradientStripState, event: &Event, ctx: &mut EventCtx<'_>) {
        match state.grab.update(event, ctx) {
            Some(GrabStep::Moved { held, to }) => return self.dragged(state, held, to, ctx),
            Some(GrabStep::Ended { held, at }) => {
                if let (true, Some(edit)) = (state.moved, self.drawn_to(held, at, ctx.bounds())) {
                    ctx.act(&self.on_commit, (edit,));
                }
                return;
            }
            Some(GrabStep::Cancelled { held }) => {
                if let (true, Some(edit)) = (state.moved, self.settled(held)) {
                    ctx.act(&self.on_commit, (edit,));
                }
                return;
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
                self.pressed(state, position, ctx);
            }
            Event::KeyPressed { key, modifiers, .. } if ctx.focused() => {
                self.keyed(key, modifiers, ctx);
            }
            _ => {}
        }
    }

    fn same(&self, other: &GradientStrip) -> bool {
        self.stops == other.stops
            && self.selected == other.selected
            && self.on_change == other.on_change
            && self.on_commit == other.on_commit
    }

    fn behavior(&self, _state: &GradientStripState) -> Behavior {
        Behavior::FOCUSABLE
    }
}

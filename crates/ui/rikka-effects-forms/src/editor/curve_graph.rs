use rikka_types::Color;
use rikka_ui::{
    Action, Behavior, Event, EventCtx, IntoHandler, Key, Layout, Length, Modifiers, PaintCtx,
    Point, PointerButton, Quad, Rect, Widget,
};

use rikka_widgets::{Grab, GrabStep};

use super::plot::{draw_bins, draw_graph, draw_grid};
use rikka_effects::{Curve, spline_of};

const SIDE: f32 = 248.0;
const POINT: f32 = 8.0;
const REACH: f32 = 7.0;
const OUTSIDE: f32 = 24.0;
const NUDGE: f32 = 1.0 / 255.0;
const LARGE_NUDGE: f32 = 10.0 / 255.0;
const DIVISIONS: usize = 4;
const HISTOGRAM_ALPHA: f32 = 0.22;
const FAINT_ALPHA: f32 = 0.45;
const BASELINE_ALPHA: f32 = 0.35;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum CurveEdit {
    Select(usize),
    Insert(f32, f32),
    Move(usize, f32, f32),
    Remove(usize),
}

#[derive(Widget)]
pub(crate) struct CurveGraph {
    #[debug]
    channel: usize,
    curves: [Vec<(f32, f32)>; 4],
    selected: Option<usize>,
    bins: Option<Vec<f32>>,
    on_change: Action<(CurveEdit,)>,
    on_commit: Action<(CurveEdit,)>,
}

#[derive(Debug, Default)]
pub(crate) struct CurveGraphState {
    grab: Grab<usize>,
    moved: bool,
}

fn channel_color(channel: usize, composite: Color) -> Color {
    match channel {
        1 => Color::rgb(0.92, 0.26, 0.26),
        2 => Color::rgb(0.22, 0.75, 0.32),
        3 => Color::rgb(0.25, 0.48, 0.96),
        _ => composite,
    }
}

impl CurveGraph {
    pub(crate) fn new<M, N>(
        channel: usize,
        curves: [Vec<(f32, f32)>; 4],
        on_change: impl IntoHandler<(CurveEdit,), (), M>,
        on_commit: impl IntoHandler<(CurveEdit,), (), N>,
    ) -> CurveGraph {
        CurveGraph {
            channel,
            curves,
            selected: None,
            bins: None,
            on_change: Action::new(on_change),
            on_commit: Action::new(on_commit),
        }
    }

    pub(crate) fn selected(mut self, selected: Option<usize>) -> CurveGraph {
        self.selected = selected;
        self
    }

    pub(crate) fn histogram(mut self, bins: Option<Vec<f32>>) -> CurveGraph {
        self.bins = bins;
        self
    }

    fn points(&self) -> &[(f32, f32)] {
        &self.curves[self.channel]
    }

    fn square(bounds: Rect) -> Rect {
        let side = bounds.width().min(bounds.height());
        Rect::new(
            bounds.x() + (bounds.width() - side) / 2.0,
            bounds.y(),
            side,
            side,
        )
    }

    fn value_at(square: Rect, position: Point) -> (f32, f32) {
        (
            ((position.x - square.x()) / square.width()).clamp(0.0, 1.0),
            ((square.max_y() - position.y) / square.height()).clamp(0.0, 1.0),
        )
    }

    fn place(square: Rect, (x, y): (f32, f32)) -> Point {
        Point::new(
            square.x() + x * square.width(),
            square.max_y() - y * square.height(),
        )
    }

    fn hit(&self, square: Rect, position: Point) -> Option<usize> {
        self.points()
            .iter()
            .enumerate()
            .map(|(index, &point)| {
                let at = CurveGraph::place(square, point);
                (index, (at.x - position.x).hypot(at.y - position.y))
            })
            .filter(|(_, distance)| *distance <= REACH)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(index, _)| index)
    }

    fn pressed(&self, state: &mut CurveGraphState, position: Point, ctx: &mut EventCtx<'_>) {
        let square = CurveGraph::square(ctx.bounds());
        let edit = match self.hit(square, position) {
            Some(index) => Some((index, CurveEdit::Select(index))),
            None if square.contains(position) => {
                let (x, y) = CurveGraph::value_at(square, position);
                Curve::insertion(self.points(), x).map(|index| (index, CurveEdit::Insert(x, y)))
            }
            None => None,
        };
        if let Some((index, edit)) = edit {
            state.moved = false;
            state.grab.begin(index, ctx);
            ctx.act(&self.on_commit, (edit,));
        }
    }

    fn dragged(
        &self,
        state: &mut CurveGraphState,
        index: usize,
        position: Point,
        ctx: &mut EventCtx<'_>,
    ) {
        let square = CurveGraph::square(ctx.bounds());
        let outside = position.x < square.x() - OUTSIDE
            || position.x > square.max_x() + OUTSIDE
            || position.y < square.y() - OUTSIDE
            || position.y > square.max_y() + OUTSIDE;
        if outside && Curve::removable(self.points(), index) {
            state.grab.end(ctx);
            ctx.act(&self.on_commit, (CurveEdit::Remove(index),));
            return;
        }
        state.moved = true;
        let (x, y) = CurveGraph::value_at(square, position);
        ctx.act(&self.on_change, (CurveEdit::Move(index, x, y),));
    }

    fn keyed(&self, key: Key, modifiers: Modifiers, ctx: &mut EventCtx<'_>) {
        let Some(index) = self.selected else {
            return;
        };
        let Some(&(x, y)) = self.points().get(index) else {
            return;
        };
        let step = match modifiers.shift {
            true => LARGE_NUDGE,
            false => NUDGE,
        };
        let edit = match key {
            Key::Delete if Curve::removable(self.points(), index) => CurveEdit::Remove(index),
            Key::ArrowLeft => CurveEdit::Move(index, x - step, y),
            Key::ArrowRight => CurveEdit::Move(index, x + step, y),
            Key::ArrowUp => CurveEdit::Move(index, x, y + step),
            Key::ArrowDown => CurveEdit::Move(index, x, y - step),
            _ => return,
        };
        ctx.set_handled();
        ctx.act(&self.on_commit, (edit,));
    }
}

impl Widget for CurveGraph {
    type State = CurveGraphState;

    fn layout(&self) -> Layout {
        Layout::column()
            .size(Length::FILL, Length::px(SIDE))
            .fixed()
    }

    fn paint(&self, _state: &mut CurveGraphState, ctx: &mut PaintCtx<'_>) {
        let palette = ctx.palette();
        let square = CurveGraph::square(ctx.bounds());
        ctx.draw_quad(Quad::new(square).fill(palette.surface_variant));
        if let Some(bins) = &self.bins {
            draw_bins(
                ctx,
                square,
                bins,
                palette.text_muted.scale_alpha(HISTOGRAM_ALPHA),
            );
        }
        draw_grid(ctx, square, DIVISIONS, palette.outline);
        draw_graph(
            ctx,
            square,
            |x| x,
            palette.text_muted.scale_alpha(BASELINE_ALPHA),
            1.0,
        );
        for (channel, points) in self.curves.iter().enumerate() {
            if channel == self.channel || points.as_slice() == [(0.0, 0.0), (1.0, 1.0)] {
                continue;
            }
            let spline = spline_of(points);
            draw_graph(
                ctx,
                square,
                |x| spline.evaluate(f64::from(x)) as f32,
                channel_color(channel, palette.text).scale_alpha(FAINT_ALPHA),
                1.0,
            );
        }
        let color = channel_color(self.channel, palette.text);
        let spline = spline_of(self.points());
        draw_graph(
            ctx,
            square,
            |x| spline.evaluate(f64::from(x)) as f32,
            color,
            2.0,
        );
        for (index, &point) in self.points().iter().enumerate() {
            let at = CurveGraph::place(square, point);
            let fill = match Some(index) == self.selected {
                true => color,
                false => palette.surface,
            };
            ctx.draw_quad(
                Quad::new(Rect::new(
                    at.x - POINT / 2.0,
                    at.y - POINT / 2.0,
                    POINT,
                    POINT,
                ))
                .fill(fill)
                .stroke(1.5, color),
            );
        }
        let edge = match ctx.focused() {
            true => palette.accent,
            false => palette.outline,
        };
        ctx.draw_quad(Quad::new(square).stroke(1.0, edge));
    }

    fn event(&self, state: &mut CurveGraphState, event: &Event, ctx: &mut EventCtx<'_>) {
        let square = CurveGraph::square(ctx.bounds());
        match state.grab.update(event, ctx) {
            Some(GrabStep::Moved { held, to }) => return self.dragged(state, held, to, ctx),
            Some(GrabStep::Ended { held, at }) => {
                if state.moved {
                    let (x, y) = CurveGraph::value_at(square, at);
                    ctx.act(&self.on_commit, (CurveEdit::Move(held, x, y),));
                }
                return;
            }
            Some(GrabStep::Cancelled { held }) => {
                if let (true, Some(&(x, y))) = (state.moved, self.points().get(held)) {
                    ctx.act(&self.on_commit, (CurveEdit::Move(held, x, y),));
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
                ctx.set_handled();
                self.pressed(state, position, ctx);
            }
            Event::KeyPressed { key, modifiers, .. } if ctx.focused() => {
                self.keyed(key, modifiers, ctx);
            }
            _ => {}
        }
    }

    fn same(&self, other: &CurveGraph) -> bool {
        self.channel == other.channel
            && self.curves == other.curves
            && self.selected == other.selected
            && self.bins == other.bins
            && self.on_change == other.on_change
            && self.on_commit == other.on_commit
    }

    fn behavior(&self, _state: &CurveGraphState) -> Behavior {
        Behavior::FOCUSABLE
    }
}

use std::ops::RangeInclusive;

use rikka_app::prelude::*;
use rikka_physics_tailor::Curve;

const LINE: f32 = 1.0;
const STROKE: f32 = 2.0;
const KNOB: f32 = 9.0;
const REACH: f32 = 10.0;
const GRID: f32 = 0.25;
const KNOB_TINT: Color = Color::rgb(1.0, 0.3, 0.3);

type Points = Vec<[f32; 2]>;

pub(super) struct DepthCurve {
    curve: Curve,
    range: RangeInclusive<f32>,
    on_shape: Handler<(Curve,)>,
}

#[derive(Debug, Default)]
pub(super) struct Shaping {
    grab: Option<(usize, Points)>,
}

impl DepthCurve {
    pub(super) fn new<Marker>(
        curve: Curve,
        range: RangeInclusive<f32>,
        on_shape: impl IntoHandler<(Curve,), (), Marker>,
    ) -> Self {
        Self {
            curve,
            range,
            on_shape: on_shape.into_handler(),
        }
    }

    fn plot(bounds: Rect) -> Rect {
        Rect::new(
            bounds.x() + KNOB / 2.0,
            bounds.y() + KNOB / 2.0,
            (bounds.width() - KNOB).max(1.0),
            (bounds.height() - KNOB).max(1.0),
        )
    }

    fn spot(&self, plot: Rect, [depth, value]: [f32; 2]) -> Point {
        let (low, high) = (*self.range.start(), *self.range.end());
        let share = ((value - low) / (high - low).max(f32::EPSILON)).clamp(0.0, 1.0);
        Point::new(
            plot.x() + depth * plot.width(),
            plot.max_y() - share * plot.height(),
        )
    }

    fn reading(&self, plot: Rect, at: Point) -> [f32; 2] {
        let (low, high) = (*self.range.start(), *self.range.end());
        let depth = ((at.x - plot.x()) / plot.width()).clamp(0.0, 1.0);
        let share = ((plot.max_y() - at.y) / plot.height()).clamp(0.0, 1.0);
        [depth, low + share * (high - low)]
    }

    fn near(&self, plot: Rect, points: &[[f32; 2]], at: Point) -> Option<usize> {
        points
            .iter()
            .enumerate()
            .map(|(index, point)| {
                let spot = self.spot(plot, *point);
                (index, (spot.x - at.x).hypot(spot.y - at.y))
            })
            .filter(|(_, away)| *away <= REACH)
            .min_by(|one, two| one.1.total_cmp(&two.1))
            .map(|(index, _)| index)
    }
}

fn placed(points: &[[f32; 2]], index: usize, [depth, value]: [f32; 2]) -> Points {
    let mut placed = points.to_vec();
    let low = index.checked_sub(1).map_or(0.0, |before| points[before][0]);
    let high = points.get(index + 1).map_or(1.0, |after| after[0]);
    placed[index] = [depth.clamp(low, high), value];
    placed
}

fn added(points: &[[f32; 2]], point: [f32; 2]) -> (Points, usize) {
    let index = points.partition_point(|other| other[0] < point[0]);
    let mut added = points.to_vec();
    added.insert(index, point);
    (added, index)
}

fn removed(points: &[[f32; 2]], index: usize) -> Option<Points> {
    (points.len() > 1).then(|| {
        let mut removed = points.to_vec();
        removed.remove(index);
        removed
    })
}

impl Widget for DepthCurve {
    type State = Shaping;

    fn paint(&self, state: &mut Shaping, ctx: &mut PaintCtx<'_>) {
        let palette = palette();
        let plot = Self::plot(ctx.bounds());
        let points: &[[f32; 2]] = state
            .grab
            .as_ref()
            .map_or(self.curve.points(), |(_, points)| points.as_slice());
        ctx.draw_quad(
            Quad::new(plot)
                .fill(palette.surface)
                .stroke(LINE, palette.outline),
        );
        let mut share = GRID;
        while share < 1.0 {
            let rule = palette.outline.scale_alpha(0.35);
            let x = plot.x() + plot.width() * share;
            let y = plot.y() + plot.height() * share;
            ctx.draw_quad(Quad::new(Rect::new(x, plot.y(), LINE, plot.height())).fill(rule));
            ctx.draw_quad(Quad::new(Rect::new(plot.x(), y, plot.width(), LINE)).fill(rule));
            share += GRID;
        }
        let curve = Curve::from_points(points.iter().copied());
        let columns = plot.width().floor().max(1.0) as usize;
        let height = |column: usize| {
            let depth = column as f32 / columns as f32;
            self.spot(plot, [depth, curve.sample(depth)]).y
        };
        let mut before = height(0);
        for column in 0..columns {
            let after = height(column + 1);
            let top = before.min(after) - STROKE / 2.0;
            let bottom = before.max(after) + STROKE / 2.0;
            ctx.draw_quad(
                Quad::new(Rect::new(plot.x() + column as f32, top, 1.0, bottom - top))
                    .fill(palette.accent),
            );
            before = after;
        }
        for (index, point) in points.iter().enumerate() {
            let lifted = state
                .grab
                .as_ref()
                .is_some_and(|(grabbed, _)| *grabbed == index);
            let size = if lifted { KNOB + 2.0 } else { KNOB };
            let spot = self.spot(plot, *point);
            ctx.draw_circle(spot, size, KNOB_TINT);
            ctx.draw_circle(spot, size - 4.0, palette.surface);
        }
    }

    fn event(&self, state: &mut Shaping, event: &Event, ctx: &mut EventCtx<'_>) {
        let plot = Self::plot(ctx.bounds());
        let points = self.curve.points();
        match *event {
            Event::PointerPressed {
                position,
                button: PointerButton::Primary,
                ..
            } => {
                let grabbed = match self.near(plot, points, position) {
                    Some(index) => (index, points.to_vec()),
                    None => {
                        let (added, index) = added(points, self.reading(plot, position));
                        (index, added)
                    }
                };
                state.grab = Some(grabbed);
                ctx.capture_pointer();
                ctx.request_paint();
            }
            Event::PointerPressed {
                position,
                button: PointerButton::Secondary,
                ..
            } => {
                let thinned = self
                    .near(plot, points, position)
                    .and_then(|index| removed(points, index));
                if let Some(thinned) = thinned {
                    ctx.call(&self.on_shape, (Curve::from_points(thinned),));
                }
            }
            Event::PointerMoved(position) => {
                if let Some((index, points)) = &state.grab {
                    let index = *index;
                    let moved = placed(points, index, self.reading(plot, position));
                    state.grab = Some((index, moved));
                    ctx.request_paint();
                }
            }
            Event::PointerReleased { .. } => {
                if let Some((_, points)) = state.grab.take() {
                    ctx.release_pointer();
                    ctx.request_paint();
                    let points = Curve::from_points(points);
                    if points != self.curve {
                        ctx.call(&self.on_shape, (points,));
                    }
                }
            }
            _ => {}
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.curve == other.curve && self.range == other.range
    }

    fn debug_name(&self) -> &'static str {
        "DepthCurve"
    }
}

impl std::fmt::Debug for DepthCurve {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DepthCurve")
            .field("curve", &self.curve)
            .finish_non_exhaustive()
    }
}

use animate_ui::{centred, dotted};
use rikka_app::prelude::*;
use rikka_model::mmd::VmdCurve;
use rikka_timeline::CurvePlot;

const GRID: f32 = 0.25;
const LINE: f32 = 1.0;
const STROKE: f32 = 2.0;
const KNOB: f32 = 10.0;
const NUMBERS: f32 = 11.5;
const NUMBERS_LINE: f32 = 16.0;
const SAMPLES: usize = 96;
const KNOB_TINT: Color = Color::rgb(1.0, 0.3, 0.3);

fn span() -> f32 {
    f32::from(VmdCurve::SPAN)
}

fn paint_curve(ctx: &mut PaintCtx<'_>, plot: &CurvePlot, curve: VmdCurve, tint: Color) {
    let points: Vec<Point> = (0..=SAMPLES)
        .map(|step| {
            let x = step as f32 / SAMPLES as f32;
            Point::new(x, curve.factor(x))
        })
        .collect();
    plot.line(ctx, &points, STROKE, tint);
}

fn knobs(curve: VmdCurve) -> [Point; 2] {
    let place = |(x, y): (u8, u8)| Point::new(f32::from(x) / span(), f32::from(y) / span());
    [place(curve.first), place(curve.second)]
}

pub(crate) struct CurveEditor {
    curve: VmdCurve,
    on_bend: Handler<(VmdCurve,)>,
}

#[derive(Debug, Default)]
pub(crate) struct Bending {
    grab: Option<(usize, VmdCurve)>,
}

impl CurveEditor {
    pub(crate) fn new<Marker>(
        curve: VmdCurve,
        on_bend: impl IntoHandler<(VmdCurve,), (), Marker>,
    ) -> Self {
        Self {
            curve,
            on_bend: on_bend.into_handler(),
        }
    }

    fn plot(bounds: Rect) -> Rect {
        let side = bounds.width().min(bounds.height() - NUMBERS_LINE).max(0.0) - KNOB;
        Rect::new(
            bounds.x() + (bounds.width() - side) / 2.0,
            bounds.y() + KNOB / 2.0,
            side,
            side,
        )
    }
}

fn dragged(curve: VmdCurve, knob: usize, plot: &CurvePlot, at: Point) -> VmdCurve {
    let byte = |share: f32| (share.clamp(0.0, 1.0) * span()).round() as u8;
    let share = plot.unplace(at);
    let place = (byte(share.x), byte(share.y));
    match knob {
        0 => VmdCurve {
            first: place,
            ..curve
        },
        _ => VmdCurve {
            second: place,
            ..curve
        },
    }
}

impl Widget for CurveEditor {
    type State = Bending;

    fn paint(&self, state: &mut Bending, ctx: &mut PaintCtx<'_>) {
        let palette = palette();
        let bounds = ctx.bounds();
        let plot = Self::plot(bounds);
        let curve = state.grab.map_or(self.curve, |(_, curve)| curve);

        ctx.draw_quad(
            Quad::new(plot)
                .fill(palette.surface)
                .stroke(LINE, palette.outline),
        );
        let mut share = GRID;
        while share < 1.0 {
            let x = plot.x() + plot.width() * share;
            let y = plot.y() + plot.height() * share;
            let rule = palette.outline.scale_alpha(0.35);
            ctx.draw_quad(Quad::new(Rect::new(x, plot.y(), LINE, plot.height())).fill(rule));
            ctx.draw_quad(Quad::new(Rect::new(plot.x(), y, plot.width(), LINE)).fill(rule));
            share += GRID;
        }
        let unit = CurvePlot::unit(plot);
        let [first, second] = knobs(curve);
        let corner = (
            Point::new(plot.x(), plot.max_y()),
            Point::new(plot.max_x(), plot.y()),
        );
        dotted(ctx, corner.0, unit.place(first), KNOB_TINT.scale_alpha(0.7));
        dotted(
            ctx,
            corner.1,
            unit.place(second),
            KNOB_TINT.scale_alpha(0.7),
        );
        paint_curve(ctx, &unit, curve, palette.accent);
        for (index, knob) in [first, second].into_iter().enumerate() {
            let held = state.grab.is_some_and(|(grabbed, _)| grabbed == index);
            unit.knob(ctx, knob, held, KNOB_TINT);
        }

        let [a, b, c, d] = curve.to_bytes();
        let numbers = format!("({a}, {b})  ({c}, {d})");
        centred(
            ctx,
            Rect::new(
                bounds.x(),
                bounds.max_y() - NUMBERS_LINE,
                bounds.width(),
                NUMBERS_LINE,
            ),
            TextRun::new(numbers, NUMBERS),
            NUMBERS_LINE,
            palette.text_muted,
        );
    }

    fn event(&self, state: &mut Bending, event: &Event, ctx: &mut EventCtx<'_>) {
        let plot = CurvePlot::unit(Self::plot(ctx.bounds()));
        match *event {
            Event::PointerPressed {
                position,
                button: PointerButton::Primary,
                ..
            } => {
                if let Some(index) = plot.knob_at(&knobs(self.curve), position) {
                    state.grab = Some((index, self.curve));
                    ctx.capture_pointer();
                    ctx.request_paint();
                }
            }
            Event::PointerMoved(position) => {
                if let Some((index, curve)) = state.grab {
                    state.grab = Some((index, dragged(curve, index, &plot, position)));
                    ctx.request_paint();
                }
            }
            Event::PointerReleased { .. } => {
                if let Some((_, curve)) = state.grab.take() {
                    ctx.release_pointer();
                    ctx.request_paint();
                    if curve != self.curve {
                        ctx.call(&self.on_bend, (curve,));
                    }
                }
            }
            _ => {}
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.curve == other.curve
    }

    fn debug_name(&self) -> &'static str {
        "CurveEditor"
    }
}

impl std::fmt::Debug for CurveEditor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CurveEditor")
            .field("curve", &self.curve)
            .finish_non_exhaustive()
    }
}

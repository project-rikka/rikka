use animate_ui::{centred, stroke};
use rikka_app::prelude::*;

const RIM: f32 = 1.5;
const TICK: f32 = 6.0;
const MINOR_TICK: f32 = 3.0;
const TICKS: usize = 16;
const SHAFT: f32 = 2.5;
const REACH: f32 = 0.72;
const HEAD: f32 = 10.0;
const HEAD_SPREAD: f32 = 0.5;
const HUB: f32 = 5.0;
const LABEL: f32 = 11.0;
const LABEL_LINE: f32 = 14.0;

pub(super) struct Compass {
    yaw: f32,
    front: Label,
    on_turn: Handler<(f32,)>,
    on_turned: Handler<(f32,)>,
}

#[derive(Debug, Default)]
pub(super) struct Turning {
    grab: Option<f32>,
}

impl Compass {
    pub(super) fn new<TurnMarker, TurnedMarker>(
        yaw: f32,
        front: Label,
        on_turn: impl IntoHandler<(f32,), (), TurnMarker>,
        on_turned: impl IntoHandler<(f32,), (), TurnedMarker>,
    ) -> Self {
        Self {
            yaw,
            front,
            on_turn: on_turn.into_handler(),
            on_turned: on_turned.into_handler(),
        }
    }
}

fn dial(bounds: Rect) -> (Point, f32) {
    (
        bounds.center(),
        bounds.width().min(bounds.height()) * 0.5 - RIM,
    )
}

fn heading(yaw: f32) -> Point {
    let radians = yaw.to_radians();
    Point::new(-radians.sin(), -radians.cos())
}

fn yaw_toward(centre: Point, at: Point) -> Option<f32> {
    let (across, down) = (at.x - centre.x, at.y - centre.y);
    (across.hypot(down) >= 1.0).then(|| (-across).atan2(-down).to_degrees().round())
}

impl Widget for Compass {
    type State = Turning;

    fn paint(&self, state: &mut Turning, ctx: &mut PaintCtx<'_>) {
        let palette = palette();
        let (centre, radius) = dial(ctx.bounds());
        if radius <= HEAD {
            return;
        }
        ctx.draw_quad(
            Quad::new(Rect::from_circle(centre, radius))
                .radius(radius)
                .fill(palette.surface),
        );
        ctx.draw_ring(Ring::new(centre, radius * 2.0).stroke(RIM, palette.outline));
        for tick in 0..TICKS {
            let along = heading(360.0 * tick as f32 / TICKS as f32);
            let length = match tick % (TICKS / 4) {
                0 => TICK,
                _ => MINOR_TICK,
            };
            stroke(
                ctx,
                centre + along * radius,
                centre + along * (radius - length),
                1.0,
                palette.outline,
            );
        }
        centred(
            ctx,
            Rect::new(
                centre.x - radius,
                centre.y + radius - TICK - LABEL_LINE,
                radius * 2.0,
                LABEL_LINE,
            ),
            TextRun::new(self.front.to_string(), LABEL),
            LABEL_LINE,
            palette.text_muted,
        );
        let along = heading(state.grab.unwrap_or(self.yaw));
        let tip = centre + along * radius * REACH;
        let tail = centre - along * radius * REACH;
        stroke(ctx, tail, tip, SHAFT, palette.accent);
        for side in [-HEAD_SPREAD, HEAD_SPREAD] {
            let (sin, cos) = side.sin_cos();
            let back = Point::new(-along.x, -along.y);
            let barb = Point::new(back.x * cos - back.y * sin, back.x * sin + back.y * cos);
            stroke(ctx, tip, tip + barb * HEAD, SHAFT, palette.accent);
        }
        ctx.draw_circle(centre, HUB, palette.accent);
    }

    fn event(&self, state: &mut Turning, event: &Event, ctx: &mut EventCtx<'_>) {
        let (centre, radius) = dial(ctx.bounds());
        match *event {
            Event::PointerPressed {
                position,
                button: PointerButton::Primary,
                ..
            } => {
                if position.distance(centre) > radius {
                    return;
                }
                let Some(yaw) = yaw_toward(centre, position) else {
                    return;
                };
                state.grab = Some(yaw);
                ctx.capture_pointer();
                ctx.request_paint();
                ctx.call(&self.on_turn, (yaw,));
            }
            Event::PointerMoved(position) => {
                if state.grab.is_none() {
                    return;
                }
                if let Some(yaw) = yaw_toward(centre, position) {
                    state.grab = Some(yaw);
                    ctx.request_paint();
                    ctx.call(&self.on_turn, (yaw,));
                }
            }
            Event::PointerReleased { .. } => {
                if let Some(yaw) = state.grab.take() {
                    ctx.release_pointer();
                    ctx.request_paint();
                    ctx.call(&self.on_turned, (yaw,));
                }
            }
            _ => {}
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.yaw == other.yaw && self.front == other.front
    }

    fn debug_name(&self) -> &'static str {
        "Compass"
    }
}

impl std::fmt::Debug for Compass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Compass")
            .field("yaw", &self.yaw)
            .finish_non_exhaustive()
    }
}

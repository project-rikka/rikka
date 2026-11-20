use animate_solve::EyeDirection;
use animate_ui::{centred, disc, stroke};
use rikka_app::prelude::*;

const ROUNDING: f32 = 10.0;
const RIM: f32 = 1.5;
const IRIS: f32 = 9.0;
const PUPIL: f32 = 4.0;
const GLINT: f32 = 1.8;
const CROSS: f32 = 1.0;
const INSET: f32 = 4.0;
const CAPTION: f32 = 11.0;
const CAPTION_LINE: f32 = 14.0;

pub(super) struct Pad {
    eye_direction: EyeDirection,
    caption: Option<Label>,
    on_eye_direction: Handler<(EyeDirection,)>,
}

#[derive(Debug, Default)]
pub(super) struct Aiming {
    grab: Option<EyeDirection>,
}

impl Pad {
    pub(super) fn new<Marker>(
        eye_direction: EyeDirection,
        caption: Option<Label>,
        on_eye_direction: impl IntoHandler<(EyeDirection,), (), Marker>,
    ) -> Self {
        Self {
            eye_direction,
            caption,
            on_eye_direction: on_eye_direction.into_handler(),
        }
    }
}

fn travel(bounds: Rect) -> (f32, f32) {
    (
        (bounds.width() / 2.0 - IRIS).max(1.0),
        (bounds.height() / 2.0 - IRIS).max(1.0),
    )
}

fn eye_direction_at(bounds: Rect, at: Point) -> EyeDirection {
    let centre = bounds.center();
    let (across, down) = travel(bounds);
    EyeDirection {
        across: ((at.x - centre.x) / across).clamp(-1.0, 1.0),
        up: (-(at.y - centre.y) / down).clamp(-1.0, 1.0),
    }
}

fn spot(bounds: Rect, eye_direction: EyeDirection) -> Point {
    let centre = bounds.center();
    let (across, down) = travel(bounds);
    Point::new(
        centre.x + eye_direction.across * across,
        centre.y - eye_direction.up * down,
    )
}

impl Widget for Pad {
    type State = Aiming;

    fn paint(&self, state: &mut Aiming, ctx: &mut PaintCtx<'_>) {
        let palette = palette();
        let bounds = ctx.bounds();
        let centre = bounds.center();
        ctx.draw_quad(
            Quad::new(bounds)
                .radius(ROUNDING)
                .fill(palette.surface_variant)
                .stroke(RIM, palette.outline),
        );
        stroke(
            ctx,
            Point::new(centre.x, bounds.y() + INSET),
            Point::new(centre.x, bounds.max_y() - INSET),
            CROSS,
            palette.outline,
        );
        stroke(
            ctx,
            Point::new(bounds.x() + INSET, centre.y),
            Point::new(bounds.max_x() - INSET, centre.y),
            CROSS,
            palette.outline,
        );
        if let Some(caption) = &self.caption {
            centred(
                ctx,
                Rect::new(bounds.x(), bounds.y() + INSET, bounds.width(), CAPTION_LINE),
                TextRun::new(caption.to_string(), CAPTION),
                CAPTION_LINE,
                palette.text_muted,
            );
        }
        let dragging = state.grab.is_some();
        let at = spot(bounds, state.grab.unwrap_or(self.eye_direction));
        let iris = match dragging {
            true => palette.warning,
            false => palette.accent,
        };
        ctx.draw_quad(
            disc(at, IRIS)
                .fill(iris)
                .stroke(RIM, Color::WHITE.scale_alpha(0.8)),
        );
        ctx.draw_quad(disc(at, PUPIL).fill(Color::BLACK.scale_alpha(0.85)));
        let glint = Point::new(at.x - PUPIL * 0.6, at.y - PUPIL * 0.6);
        ctx.draw_quad(disc(glint, GLINT).fill(Color::WHITE.scale_alpha(0.9)));
    }

    fn event(&self, state: &mut Aiming, event: &Event, ctx: &mut EventCtx<'_>) {
        let bounds = ctx.bounds();
        match *event {
            Event::PointerPressed {
                position,
                button: PointerButton::Primary,
                ..
            } => {
                if ctx.presses() >= 2 {
                    state.grab = None;
                    ctx.request_paint();
                    ctx.call(&self.on_eye_direction, (EyeDirection::default(),));
                    return;
                }
                let eye_direction = eye_direction_at(bounds, position);
                state.grab = Some(eye_direction);
                ctx.capture_pointer();
                ctx.request_paint();
                ctx.call(&self.on_eye_direction, (eye_direction,));
            }
            Event::PointerMoved(position) if state.grab.is_some() => {
                let eye_direction = eye_direction_at(bounds, position);
                state.grab = Some(eye_direction);
                ctx.request_paint();
                ctx.call(&self.on_eye_direction, (eye_direction,));
            }
            Event::PointerReleased { .. } if state.grab.take().is_some() => {
                ctx.release_pointer();
                ctx.request_paint();
            }
            _ => {}
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.eye_direction == other.eye_direction && self.caption == other.caption
    }

    fn debug_name(&self) -> &'static str {
        "Pad"
    }
}

impl std::fmt::Debug for Pad {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pad")
            .field("eye_direction", &self.eye_direction)
            .finish_non_exhaustive()
    }
}

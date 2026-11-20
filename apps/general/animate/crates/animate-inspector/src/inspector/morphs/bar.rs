use rikka_app::prelude::*;
use rikka_ui::DRAG_SLOP;

const ROUNDING: f32 = 6.0;
const INSET: f32 = 8.0;
const TEXT: f32 = 12.0;
const LINE: f32 = 16.0;
const GAP: f32 = 6.0;
const FILL: f32 = 0.28;
const FILL_LIT: f32 = 0.38;
const LIT: f32 = 0.05;
const CHOSEN: f32 = 1.0;

pub(super) struct MorphBar {
    name: String,
    weight: f32,
    pending: bool,
    chosen: bool,
    on_slide: Handler<(f32,)>,
    on_pick: Handler<()>,
    on_type: Handler<()>,
}

#[derive(Debug, Default)]
pub(super) struct Sliding {
    grab: Option<Grab>,
}

#[derive(Clone, Copy, Debug)]
struct Grab {
    from: Point,
    weight: f32,
    shown: f32,
    moved: bool,
}

impl MorphBar {
    pub(super) fn new<Slide, Pick, Type>(
        name: String,
        weight: f32,
        on_slide: impl IntoHandler<(f32,), (), Slide>,
        on_pick: impl IntoHandler<(), (), Pick>,
        on_type: impl IntoHandler<(), (), Type>,
    ) -> Self {
        Self {
            name,
            weight: weight.clamp(0.0, 1.0),
            pending: false,
            chosen: false,
            on_slide: on_slide.into_handler(),
            on_pick: on_pick.into_handler(),
            on_type: on_type.into_handler(),
        }
    }

    pub(super) fn pending(mut self, pending: bool) -> Self {
        self.pending = pending;
        self
    }

    pub(super) fn chosen(mut self, chosen: bool) -> Self {
        self.chosen = chosen;
        self
    }
}

impl Widget for MorphBar {
    type State = Sliding;

    fn paint(&self, state: &mut Sliding, ctx: &mut PaintCtx<'_>) {
        let palette = palette();
        let bounds = ctx.bounds();
        let highlighted = ctx.hovered() || state.grab.is_some();
        let weight = state.grab.map_or(self.weight, |grab| grab.shown);
        let track = Quad::new(bounds)
            .radius(ROUNDING)
            .fill(palette.surface_variant);
        ctx.draw_quad(match self.chosen {
            true => track.stroke(CHOSEN, palette.accent),
            false => track,
        });
        if highlighted {
            ctx.draw_quad(
                Quad::new(bounds)
                    .radius(ROUNDING)
                    .fill(palette.text.scale_alpha(LIT)),
            );
        }
        let filled = bounds.width() * weight;
        if filled > 0.0 {
            let tint = match highlighted {
                true => FILL_LIT,
                false => FILL,
            };
            ctx.draw_quad(
                Quad::new(Rect::new(bounds.x(), bounds.y(), filled, bounds.height()))
                    .radius(ROUNDING.min(filled / 2.0))
                    .fill(palette.accent.scale_alpha(tint)),
            );
        }
        let value = TextRun::new(format!("{weight:.2}"), TEXT).no_wrap();
        let value_width = measure_text(&value, None).width;
        let line = bounds.center().y - LINE / 2.0;
        ctx.draw_text(
            Rect::new(
                bounds.max_x() - INSET - value_width,
                line,
                value_width + 1.0,
                LINE,
            ),
            value,
            palette.text_muted,
        );
        let room = (bounds.width() - INSET * 2.0 - value_width - GAP).max(0.0);
        let name = match self.pending {
            true => palette.accent,
            false => palette.text,
        };
        ctx.draw_text(
            Rect::new(bounds.x() + INSET, line, room, LINE),
            TextRun::new(self.name.clone(), TEXT).no_wrap(),
            name,
        );
    }

    fn event(&self, state: &mut Sliding, event: &Event, ctx: &mut EventCtx<'_>) {
        let bounds = ctx.bounds();
        match *event {
            Event::PointerPressed {
                position,
                button: PointerButton::Primary,
                ..
            } => {
                if ctx.presses() >= 2 {
                    state.grab = None;
                    ctx.call(&self.on_type, ());
                    return;
                }
                state.grab = Some(Grab {
                    from: position,
                    weight: self.weight,
                    shown: self.weight,
                    moved: false,
                });
                ctx.capture_pointer();
                ctx.request_paint();
            }
            Event::PointerMoved(position) => {
                let Some(grab) = state.grab.as_mut() else {
                    return;
                };
                let travel = position.x - grab.from.x;
                if !grab.moved && travel.abs() < DRAG_SLOP {
                    return;
                }
                grab.moved = true;
                let weight = (grab.weight + travel / bounds.width().max(1.0)).clamp(0.0, 1.0);
                if weight != grab.shown {
                    grab.shown = weight;
                    ctx.request_paint();
                    ctx.call(&self.on_slide, (weight,));
                }
            }
            Event::PointerReleased { .. } => {
                let Some(grab) = state.grab.take() else {
                    return;
                };
                ctx.release_pointer();
                ctx.request_paint();
                if !grab.moved {
                    ctx.call(&self.on_pick, ());
                }
            }
            _ => {}
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.name == other.name
            && self.weight == other.weight
            && self.pending == other.pending
            && self.chosen == other.chosen
    }

    fn cursor(&self, _state: &Sliding, _bounds: Rect, _point: Point) -> Option<CursorShape> {
        Some(CursorShape::ResizeEastWest)
    }

    fn behavior(&self, _state: &Sliding) -> Behavior {
        Behavior::TRACKS_HOVER
    }

    fn debug_name(&self) -> &'static str {
        "MorphBar"
    }
}

impl std::fmt::Debug for MorphBar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MorphBar")
            .field("name", &self.name)
            .field("weight", &self.weight)
            .finish_non_exhaustive()
    }
}

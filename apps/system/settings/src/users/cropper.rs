use rikka_app::prelude::*;
use rikka_ui::{Gesture, GesturePhase};
use settings_avatar::Framing;

const RING: f32 = 2.0;
const HAIR: f32 = 1.0;
const SHADE: Color = Color::rgba(0.0, 0.0, 0.0, 0.45);
const ZOOM_STEP: f32 = 1.25;
const KEY_PAN: f32 = 0.05;

pub(crate) struct Cropper {
    preview: ImageSource,
    framing: Framing,
    on_frame: Handler<(Framing,)>,
}

#[derive(Debug, Default)]
pub(crate) struct Gripping {
    drag: Option<Drag>,
    pinch: Option<f32>,
}

#[derive(Clone, Copy, Debug)]
struct Drag {
    from: Point,
    framing: Framing,
}

#[derive(Clone, Copy, Debug)]
struct Placing {
    square: Rect,
    scale: f32,
}

impl Placing {
    fn of(bounds: Rect, framing: &Framing) -> Placing {
        let side = bounds.size.min_side();
        let square = Rect::new(
            bounds.center().x - side / 2.0,
            bounds.center().y - side / 2.0,
            side,
            side,
        );
        Placing {
            square,
            scale: side / framing.side(),
        }
    }

    fn photo_at(&self, framing: &Framing, at: Point) -> Point {
        let middle = self.square.center();
        Point::new(
            framing.center().x + (at.x - middle.x) / self.scale,
            framing.center().y + (at.y - middle.y) / self.scale,
        )
    }

    fn photo_rect(&self, framing: &Framing) -> Rect {
        let middle = self.square.center();
        let photo = framing.photo();
        Rect::new(
            middle.x - framing.center().x * self.scale,
            middle.y - framing.center().y * self.scale,
            photo.width * self.scale,
            photo.height * self.scale,
        )
    }
}

impl Cropper {
    pub(crate) fn new<M>(
        preview: ImageSource,
        framing: Framing,
        on_frame: impl IntoHandler<(Framing,), (), M>,
    ) -> Self {
        Self {
            preview,
            framing,
            on_frame: on_frame.into_handler(),
        }
    }

    fn frame(&self, ctx: &mut EventCtx<'_>, framing: Framing) {
        ctx.set_handled();
        if framing != self.framing {
            ctx.call(&self.on_frame, (framing,));
        }
    }

    fn zoom_by(&self, ctx: &mut EventCtx<'_>, factor: f32, at: Point) {
        let placing = Placing::of(ctx.bounds(), &self.framing);
        let anchor = placing.photo_at(&self.framing, at);
        self.frame(
            ctx,
            self.framing.zoomed(self.framing.zoom() * factor, anchor),
        );
    }

    fn pan_by(&self, ctx: &mut EventCtx<'_>, x: f32, y: f32) {
        let step = self.framing.side() * KEY_PAN;
        self.frame(ctx, self.framing.panned(Point::new(x * step, y * step)));
    }

    fn pressed(&self, ctx: &mut EventCtx<'_>, key: Key) {
        let middle = ctx.bounds().center();
        match key {
            Key::Char('+' | '=') => self.zoom_by(ctx, ZOOM_STEP, middle),
            Key::Char('-' | '_') => self.zoom_by(ctx, ZOOM_STEP.recip(), middle),
            Key::ArrowLeft => self.pan_by(ctx, -1.0, 0.0),
            Key::ArrowRight => self.pan_by(ctx, 1.0, 0.0),
            Key::ArrowUp => self.pan_by(ctx, 0.0, -1.0),
            Key::ArrowDown => self.pan_by(ctx, 0.0, 1.0),
            _ => {}
        }
    }
}

impl Widget for Cropper {
    type State = Gripping;

    fn behavior(&self, _state: &Gripping) -> Behavior {
        Behavior::FOCUSABLE
    }

    fn paint(&self, _state: &mut Gripping, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        if bounds.is_empty() {
            return;
        }
        let palette = palette();
        let placing = Placing::of(bounds, &self.framing);
        ctx.clipped(RoundedRect::new(bounds, Corner::Panel.radius()), |ctx| {
            ctx.draw_quad(Quad::new(bounds).fill(palette.surface_variant));
            ctx.draw_image(placing.photo_rect(&self.framing), self.preview.clone());
        });
        let ring = match ctx.focused() {
            true => palette.accent,
            false => Color::WHITE,
        };
        let circle = |inset: f32| {
            let rect = placing.square.inflate(-inset);
            Quad::new(rect).radius(rect.width() / 2.0)
        };
        ctx.draw_quad(circle(-HAIR).stroke(HAIR, SHADE));
        ctx.draw_quad(circle(0.0).stroke(RING, ring));
        ctx.draw_quad(circle(RING).stroke(HAIR, SHADE));
    }

    fn event(&self, state: &mut Gripping, event: &Event, ctx: &mut EventCtx<'_>) {
        match *event {
            Event::PointerPressed {
                position,
                button: PointerButton::Primary,
                ..
            } => {
                state.drag = Some(Drag {
                    from: position,
                    framing: self.framing,
                });
                ctx.request_focus();
                ctx.capture_pointer();
                ctx.set_handled();
            }
            Event::PointerMoved(position) => {
                let Some(drag) = state.drag else {
                    return;
                };
                let scale = Placing::of(ctx.bounds(), &drag.framing).scale;
                self.frame(
                    ctx,
                    drag.framing.panned(Point::new(
                        (drag.from.x - position.x) / scale,
                        (drag.from.y - position.y) / scale,
                    )),
                );
            }
            Event::PointerReleased {
                button: PointerButton::Primary,
                ..
            } => {
                if state.drag.take().is_some() {
                    ctx.release_pointer();
                    ctx.set_handled();
                }
            }
            Event::Scroll {
                position, delta, ..
            } => self.zoom_by(ctx, ZOOM_STEP.powf(delta.zoom()), position),
            Event::Gesture {
                position,
                gesture: Gesture::Pinch { phase, scale, .. },
            } => match phase {
                GesturePhase::Began => {
                    state.pinch = Some(self.framing.zoom());
                    ctx.set_handled();
                }
                GesturePhase::Moved => {
                    if let Some(from) = state.pinch {
                        self.zoom_by(ctx, from * scale / self.framing.zoom(), position);
                    }
                }
                _ => state.pinch = None,
            },
            Event::KeyPressed { key, modifiers, .. }
                if !modifiers.control && !modifiers.alt && !modifiers.logo =>
            {
                self.pressed(ctx, key);
            }
            Event::Command(Command::ZoomIn) => self.zoom_by(ctx, ZOOM_STEP, ctx.bounds().center()),
            Event::Command(Command::ZoomOut) => {
                self.zoom_by(ctx, ZOOM_STEP.recip(), ctx.bounds().center());
            }
            _ => {}
        }
    }
}

impl std::fmt::Debug for Cropper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cropper")
            .field("framing", &self.framing)
            .finish_non_exhaustive()
    }
}

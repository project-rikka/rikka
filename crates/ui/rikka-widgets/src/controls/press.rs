use rikka_types::{Key, Point, PointerButton};
use rikka_ui::{Event, EventCtx};

#[derive(Debug, Default)]
pub(crate) struct Press {
    pressed: bool,
    inside: bool,
    origin: Point,
    generation: u32,
    declines_focus: bool,
}

impl Press {
    pub(crate) fn set_declines_focus(&mut self, declines: bool) {
        self.declines_focus = declines;
    }

    pub(crate) fn is_down(&self) -> bool {
        self.pressed && self.inside
    }

    pub(crate) fn last(&self) -> (u32, Point) {
        (self.generation, self.origin)
    }

    fn begin(&mut self, origin: Point) {
        self.origin = origin;
        self.generation = self.generation.wrapping_add(1);
    }

    pub(crate) fn update(&mut self, event: &Event, enabled: bool, ctx: &mut EventCtx<'_>) -> bool {
        if self.declines_focus && matches!(event, Event::PointerPressed { .. }) {
            ctx.keep_focus();
        }
        match event {
            Event::PointerPressed {
                button: PointerButton::Primary,
                position,
                ..
            } => {
                ctx.set_handled();
                if !enabled {
                    return false;
                }
                self.pressed = true;
                self.inside = ctx.bounds().contains(*position);
                if self.inside {
                    self.begin(*position - ctx.bounds().origin);
                }
                ctx.capture_pointer();
                ctx.request_paint();
                false
            }
            Event::PointerMoved(position) if self.pressed => {
                let inside = ctx.bounds().contains(*position);
                if self.inside != inside {
                    self.inside = inside;
                    ctx.request_paint();
                }
                false
            }
            Event::PointerReleased {
                button: PointerButton::Primary,
                position,
                ..
            } if self.pressed => {
                self.pressed = false;
                let activated = enabled && ctx.bounds().contains(*position);
                self.inside = false;
                ctx.release_pointer();
                ctx.request_paint();
                ctx.set_handled();
                activated
            }
            Event::PointerCancelled if self.pressed => {
                self.pressed = false;
                self.inside = false;
                ctx.request_paint();
                false
            }
            Event::KeyPressed {
                key: Key::Enter | Key::Space,
                ..
            } if enabled && ctx.focused() => {
                ctx.set_handled();
                self.begin(ctx.bounds().center() - ctx.bounds().origin);
                ctx.request_paint();
                true
            }
            _ => false,
        }
    }
}

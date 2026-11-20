use rikka_types::{Point, PointerButton};
use rikka_ui::{Event, EventCtx};

#[derive(Debug)]
pub struct Grab<T = ()> {
    held: Option<T>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GrabStep<T = ()> {
    Moved { held: T, to: Point },
    Ended { held: T, at: Point },
    Cancelled { held: T },
}

impl<T> Default for Grab<T> {
    fn default() -> Self {
        Self { held: None }
    }
}

impl<T: Copy> Grab<T> {
    pub fn held(&self) -> Option<T> {
        self.held
    }

    pub fn is_held(&self) -> bool {
        self.held.is_some()
    }

    pub fn begin(&mut self, held: T, ctx: &mut EventCtx<'_>) {
        self.held = Some(held);
        ctx.capture_pointer();
        ctx.set_handled();
        ctx.request_paint();
    }

    pub fn end(&mut self, ctx: &mut EventCtx<'_>) {
        if self.held.take().is_some() {
            ctx.release_pointer();
            ctx.request_paint();
        }
    }

    pub fn update(&mut self, event: &Event, ctx: &mut EventCtx<'_>) -> Option<GrabStep<T>> {
        let held = self.held?;
        match *event {
            Event::PointerMoved(to) => {
                ctx.set_handled();
                Some(GrabStep::Moved { held, to })
            }
            Event::PointerReleased {
                button: PointerButton::Primary,
                position,
                ..
            } => {
                self.end(ctx);
                ctx.set_handled();
                Some(GrabStep::Ended { held, at: position })
            }
            Event::PointerCancelled => {
                self.held = None;
                ctx.request_paint();
                Some(GrabStep::Cancelled { held })
            }
            _ => None,
        }
    }
}

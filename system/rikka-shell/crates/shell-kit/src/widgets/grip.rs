use rikka_types::{Point, PointerButton, Size};
use rikka_ui::{Element, Event, EventCtx, Layout, Length, PaintCtx, View, Widget};

use crate::widgets::spot::Spot;

const THRESHOLD: f32 = 3.0;

pub(crate) struct Grip {
    child: Element,
    spot: Spot,
}

#[derive(Clone, Copy, Debug)]
struct Hold {
    grab: Point,
    from: Point,
    moving: bool,
}

#[derive(Debug, Default)]
pub(crate) struct GripState {
    hold: Option<Hold>,
}

pub(crate) fn grip(child: impl View, spot: Spot) -> Grip {
    Grip {
        child: child.build(),
        spot,
    }
}

impl Widget for Grip {
    type State = GripState;

    fn layout(&self) -> Layout {
        Layout::column().width(Length::FILL)
    }

    fn paint(&self, _state: &mut GripState, ctx: &mut PaintCtx<'_>) {
        let Some(at) = self.spot.at() else {
            return;
        };
        let settled = inside(at, ctx.bounds().size, ctx.viewport());
        if settled == at {
            return;
        }
        self.spot.put(settled);
        ctx.request_view();
    }

    fn event(&self, state: &mut GripState, event: &Event, ctx: &mut EventCtx<'_>) {
        match *event {
            Event::PointerPressed {
                button: PointerButton::Primary,
                position,
                ..
            } => {
                state.hold = Some(Hold {
                    grab: position - ctx.bounds().origin,
                    from: position,
                    moving: false,
                });
                ctx.capture_pointer();
                ctx.set_handled();
            }
            Event::PointerMoved(position) => {
                let Some(hold) = state.hold.as_mut() else {
                    return;
                };
                ctx.set_handled();
                if !hold.moving && position.distance(hold.from) < THRESHOLD {
                    return;
                }
                hold.moving = true;
                let at = inside(position - hold.grab, ctx.bounds().size, ctx.viewport());
                if self.spot.at() != Some(at) {
                    self.spot.put(at);
                    ctx.request_view();
                }
            }
            Event::PointerReleased {
                button: PointerButton::Primary,
                ..
            } if state.hold.take().is_some() => {
                ctx.release_pointer();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn debug_name(&self) -> &'static str {
        "Grip"
    }
}

fn inside(at: Point, window: Size, room: Size) -> Point {
    if room.is_empty() || window.is_empty() {
        return at;
    }
    Point::new(
        at.x.clamp(0.0, (room.width - window.width).max(0.0)),
        at.y.clamp(0.0, (room.height - window.height).max(0.0)),
    )
}

impl std::fmt::Debug for Grip {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Grip")
            .field("spot", &self.spot)
            .finish_non_exhaustive()
    }
}

impl View for Grip {
    fn build(mut self) -> Element {
        let child = std::mem::replace(&mut self.child, Element::empty());
        Element::with_children(self, vec![child])
    }
}

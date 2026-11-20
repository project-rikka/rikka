use desk_layout::Kind;
use rikka_types::{CursorShape, Point, PointerButton, Rect};
use rikka_ui::{Element, Event, EventCtx, Layout, Widget};

use crate::session::{Offer, Session};

struct Carry {
    kind: Kind,
    session: Session,
}

#[must_use]
pub(crate) fn carry(kind: Kind, session: Session, preview: Element) -> Element {
    Element::with_children(Carry { kind, session }, vec![preview])
}

impl Widget for Carry {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column()
    }

    fn event(&self, _state: &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        let Event::PointerPressed {
            position,
            button: PointerButton::Primary,
            ..
        } = event
        else {
            return;
        };
        let bounds = ctx.bounds();
        self.session.offer(Offer {
            kind: self.kind,
            grab: Point::new(
                share(position.x - bounds.min_x(), bounds.width()),
                share(position.y - bounds.min_y(), bounds.height()),
            ),
        });
    }

    fn cursor(&self, _state: &Self::State, _bounds: Rect, _point: Point) -> Option<CursorShape> {
        Some(CursorShape::Grab)
    }

    fn debug_name(&self) -> &'static str {
        "DeskCarry"
    }
}

fn share(along: f32, length: f32) -> f32 {
    (along / length.max(1.0)).clamp(0.0, 1.0)
}

impl std::fmt::Debug for Carry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Carry").field("kind", &self.kind).finish()
    }
}

use std::hash::{DefaultHasher, Hash, Hasher};

use rikka_app::prelude::*;
use rikka_ui::{Handler, Layout};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Press {
    pub(crate) at: Point,
    pub(crate) size: Size,
    pub(crate) modifiers: Modifiers,
}

pub(crate) fn mark_of(subject: impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    subject.hash(&mut hasher);
    hasher.finish()
}

pub(crate) fn context_area(mark: u64, body: impl View, open: Handler<(Anchor,)>) -> Element {
    let open = open.map(move |(press,): (Press,)| {
        (Anchor::new(mark, AnchorEdge::BelowStart)
            .offset(press.at.x, press.at.y - press.size.height),)
    });
    right_click(
        Layout::column().size(Length::FILL, Length::HUG),
        true,
        body,
        open,
    )
    .keyed(mark)
}

pub(crate) fn right_click(
    layout: Layout,
    claims: bool,
    body: impl View,
    open: Handler<(Press,)>,
) -> Element {
    Element::with_children(
        RightClick {
            layout,
            claims,
            open,
        },
        vec![body.build()],
    )
}

struct RightClick {
    layout: Layout,
    claims: bool,
    open: Handler<(Press,)>,
}

impl Widget for RightClick {
    type State = ();

    fn layout(&self) -> Layout {
        self.layout
    }

    fn event(&self, (): &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        let Event::PointerPressed {
            position,
            button: PointerButton::Secondary,
            modifiers,
        } = *event
        else {
            return;
        };
        let bounds = ctx.bounds();
        let press = Press {
            at: Point::new(position.x - bounds.origin.x, position.y - bounds.origin.y),
            size: bounds.size,
            modifiers,
        };
        ctx.call(&self.open, (press,));
        if self.claims {
            ctx.set_handled();
        }
    }

    fn debug_name(&self) -> &'static str {
        "ContextArea"
    }
}

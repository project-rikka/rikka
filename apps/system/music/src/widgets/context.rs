use rikka_app::prelude::*;
use rikka_ui::{Element, Handler, Layout};

pub(crate) fn context_area(body: impl View, open: Handler<(Point,)>) -> Element {
    Element::with_children(ContextArea { open }, vec![body.build()])
}

pub(crate) fn aimed(aim: Handler, body: impl View) -> Element {
    Element::with_children(Aimed { aim }, vec![body.build()])
}

struct ContextArea {
    open: Handler<(Point,)>,
}

impl Widget for ContextArea {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().size(Length::FILL, Length::FILL)
    }

    fn event(&self, (): &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        let Some(position) = secondary(event) else {
            return;
        };
        let origin = ctx.bounds().origin;
        let at = Point::new(position.x - origin.x, position.y - origin.y);
        ctx.call(&self.open, (at,));
        ctx.set_handled();
    }

    fn debug_name(&self) -> &'static str {
        "ContextArea"
    }
}

struct Aimed {
    aim: Handler,
}

impl Widget for Aimed {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column()
    }

    fn event(&self, (): &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        if secondary(event).is_some() {
            ctx.call(&self.aim, ());
        }
    }

    fn debug_name(&self) -> &'static str {
        "Aimed"
    }
}

fn secondary(event: &Event) -> Option<Point> {
    match *event {
        Event::PointerPressed {
            position,
            button: PointerButton::Secondary,
            ..
        } => Some(position),
        _ => None,
    }
}

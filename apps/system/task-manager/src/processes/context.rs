use rikka_app::prelude::*;
use rikka_ui::{Element, Handler, Layout};

pub(crate) fn context_area(body: impl View, open: Handler<(Point,)>) -> Element {
    Element::with_children(ContextArea { open }, vec![body.build()])
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
        let Event::PointerPressed {
            position,
            button: PointerButton::Secondary,
            ..
        } = *event
        else {
            return;
        };
        let origin = ctx.bounds().origin;
        let at = Point::new(position.x - origin.x, position.y - origin.y);
        ctx.call(&self.open, (at,));
        ctx.set_handled();
    }

    fn debug_name(&self) -> &'static str {
        "ProcessesContextArea"
    }
}

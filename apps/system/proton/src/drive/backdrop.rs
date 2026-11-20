use rikka_app::prelude::{Event as Input, *};
use rikka_ui::{Element, Handler, Layout};

pub(crate) fn backdrop(body: Element, on_context: Handler<(Point,)>) -> Element {
    Element::with_children(Backdrop { on_context }, vec![body])
}

struct Backdrop {
    on_context: Handler<(Point,)>,
}

impl Widget for Backdrop {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().size(Length::FILL, Length::FILL)
    }

    fn event(&self, _held: &mut (), event: &Input, ctx: &mut EventCtx<'_>) {
        let Input::PointerPressed {
            position,
            button: PointerButton::Secondary,
            ..
        } = *event
        else {
            return;
        };
        let bounds = ctx.bounds();
        let at = Point::new(position.x - bounds.origin.x, position.y - bounds.origin.y);
        ctx.call(&self.on_context, (at,));
        ctx.set_handled();
    }

    fn debug_name(&self) -> &'static str {
        "DriveBackdrop"
    }
}

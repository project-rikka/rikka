use rikka_types::{Point, PointerButton};
use rikka_ui::{
    Element, Event, EventCtx, Handler, IntoHandler, Layout, Length, PaintCtx, View, Widget,
};

pub struct Scrim {
    child: Element,
    on_press: Handler,
    on_secondary_press: Option<Handler<(Point,)>>,
}

pub fn scrim<Marker>(child: impl View, on_press: impl IntoHandler<(), (), Marker>) -> Scrim {
    Scrim {
        child: child.build(),
        on_press: on_press.into_handler(),
        on_secondary_press: None,
    }
}

impl Scrim {
    #[must_use]
    pub fn on_secondary_press<Marker>(
        mut self,
        on_secondary_press: impl IntoHandler<(Point,), (), Marker>,
    ) -> Self {
        self.on_secondary_press = Some(on_secondary_press.into_handler());
        self
    }
}

impl Widget for Scrim {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row().size(Length::FILL, Length::FILL)
    }

    fn paint(&self, _state: &mut (), _ctx: &mut PaintCtx<'_>) {}

    fn event(&self, _state: &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        let Event::PointerPressed {
            button, position, ..
        } = event
        else {
            return;
        };
        match (button, &self.on_secondary_press) {
            (PointerButton::Primary, _) => {
                ctx.set_handled();
                ctx.call(&self.on_press, ());
            }
            (PointerButton::Secondary, Some(on_secondary_press)) => {
                ctx.set_handled();
                ctx.call(on_secondary_press, (*position,));
            }
            _ => {}
        }
    }

    fn debug_name(&self) -> &'static str {
        "Scrim"
    }
}

impl std::fmt::Debug for Scrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scrim").finish_non_exhaustive()
    }
}

impl View for Scrim {
    fn build(mut self) -> Element {
        let child = std::mem::replace(&mut self.child, Element::empty());
        Element::with_children(self, vec![child])
    }
}

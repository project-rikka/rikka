use rikka_types::{Point, PointerButton};
use rikka_ui::{
    Element, Event, EventCtx, Handler, IntoHandler, Layout, Length, PaintCtx, View, Widget,
};

pub struct Presses {
    child: Element,
    on_press: Option<Handler<(PointerButton, Point), bool>>,
    on_scroll: Option<Handler<(f32,)>>,
}

pub fn presses(child: impl View) -> Presses {
    Presses {
        child: child.build(),
        on_press: None,
        on_scroll: None,
    }
}

impl Presses {
    #[must_use]
    pub fn on_press<Marker>(
        mut self,
        on_press: impl IntoHandler<(PointerButton, Point), bool, Marker>,
    ) -> Self {
        self.on_press = Some(on_press.into_handler());
        self
    }

    #[must_use]
    pub fn on_scroll<Marker>(mut self, on_scroll: impl IntoHandler<(f32,), (), Marker>) -> Self {
        self.on_scroll = Some(on_scroll.into_handler());
        self
    }
}

#[derive(Debug, Default)]
pub struct PressesState {
    travelled: f32,
}

impl Widget for Presses {
    type State = PressesState;

    fn layout(&self) -> Layout {
        Layout::row().width(Length::FILL)
    }

    fn paint(&self, _state: &mut PressesState, _ctx: &mut PaintCtx<'_>) {}

    fn event(&self, state: &mut PressesState, event: &Event, ctx: &mut EventCtx<'_>) {
        match event {
            Event::PointerPressed {
                button, position, ..
            } if ctx.bounds().contains(*position) => {
                let Some(on_press) = &self.on_press else {
                    return;
                };
                if ctx.call(on_press, (*button, *position)).unwrap_or(false) {
                    ctx.set_handled();
                }
            }
            Event::Scroll {
                delta, position, ..
            } if ctx.bounds().contains(*position) => {
                let Some(on_scroll) = &self.on_scroll else {
                    return;
                };
                ctx.set_handled();
                state.travelled += delta.up();
                let detents = state.travelled.trunc();
                if detents == 0.0 {
                    return;
                }
                state.travelled -= detents;
                ctx.call(on_scroll, (detents,));
            }
            _ => {}
        }
    }

    fn debug_name(&self) -> &'static str {
        "Presses"
    }
}

impl std::fmt::Debug for Presses {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Presses").finish_non_exhaustive()
    }
}

impl View for Presses {
    fn build(mut self) -> Element {
        let child = std::mem::replace(&mut self.child, Element::empty());
        Element::with_children(self, vec![child])
    }
}

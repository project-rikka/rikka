use rikka_types::{Key, Modifiers};
use rikka_ui::{
    Behavior, Element, Event, EventCtx, Handler, IntoHandler, Layout, Length, PaintCtx, View,
    Widget,
};

pub struct Sheet {
    child: Element,
    on_key: Handler<(Key, Modifiers), bool>,
    keeps_tab: bool,
}

pub fn sheet<Marker>(
    child: impl View,
    on_key: impl IntoHandler<(Key, Modifiers), bool, Marker>,
) -> Sheet {
    Sheet {
        child: child.build(),
        on_key: on_key.into_handler(),
        keeps_tab: false,
    }
}

impl Sheet {
    pub fn keeps_tab(mut self) -> Self {
        self.keeps_tab = true;
        self
    }
}

impl Widget for Sheet {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().width(Length::FILL)
    }

    fn paint(&self, _state: &mut (), _ctx: &mut PaintCtx<'_>) {}

    fn event(&self, _state: &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        match event {
            Event::KeyPressed { key, modifiers, .. } => {
                if ctx.call(&self.on_key, (*key, *modifiers)).unwrap_or(false) {
                    ctx.set_handled();
                }
            }
            Event::PointerPressed { .. } => ctx.set_handled(),
            _ => {}
        }
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::TAKES_TAB.when(self.keeps_tab)
    }

    fn debug_name(&self) -> &'static str {
        "Sheet"
    }
}

impl std::fmt::Debug for Sheet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sheet").finish_non_exhaustive()
    }
}

impl View for Sheet {
    fn build(mut self) -> Element {
        let child = std::mem::replace(&mut self.child, Element::empty());
        Element::with_children(self, vec![child])
    }
}

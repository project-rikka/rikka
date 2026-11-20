use rikka_app::prelude::*;

pub(crate) fn inert(content: impl View) -> Element {
    Element::with_children(Inert, vec![content.build()])
}

struct Inert;

impl Widget for Inert {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().size(Length::FILL, Length::FILL)
    }

    fn behavior(&self, (): &()) -> Behavior {
        Behavior::INERT
    }

    fn same(&self, _: &Self) -> bool {
        true
    }

    fn debug_name(&self) -> &'static str {
        "Inert"
    }
}

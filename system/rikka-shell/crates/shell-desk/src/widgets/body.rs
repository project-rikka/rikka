use rikka_ui::{Behavior, Element, Layout, Style, Styled, View, Widget};

pub(crate) struct Body {
    inert: bool,
    view: Box<dyn Fn() -> Element>,
    style: Style,
}

#[must_use]
pub(crate) fn body(inert: bool, view: impl Fn() -> Element + 'static) -> Body {
    Body {
        inert,
        view: Box::new(view),
        style: Style::default(),
    }
}

impl Widget for Body {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().with(&self.style)
    }

    fn behavior(&self, (): &()) -> Behavior {
        Behavior::INERT.when(self.inert)
    }

    fn build(&self, (): &mut ()) -> Vec<Element> {
        vec![(self.view)()]
    }
}

impl Styled for Body {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl View for Body {
    fn build(self) -> Element {
        Element::new(self)
    }
}

impl std::fmt::Debug for Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Body")
            .field("inert", &self.inert)
            .finish_non_exhaustive()
    }
}

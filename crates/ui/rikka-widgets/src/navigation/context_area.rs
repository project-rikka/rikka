use rikka_types::PointerButton;
use rikka_ui::{Command, Element, Event, EventCtx, Layout, PaintCtx, Style, Styled, View, Widget};

use crate::navigation::menu::{ContextMenu, MenuItem};

type Items = Box<dyn Fn() -> Vec<MenuItem>>;

pub struct ContextArea {
    child: Option<Element>,
    items: Items,
    style: Style,
}

pub fn context_area(child: impl View, items: impl Fn() -> Vec<MenuItem> + 'static) -> ContextArea {
    ContextArea {
        child: Some(child.build()),
        items: Box::new(items),
        style: Style::default(),
    }
}

#[derive(Debug, Default)]
pub struct ContextAreaState {
    menu: ContextMenu,
}

impl Widget for ContextArea {
    type State = ContextAreaState;

    fn layout(&self) -> Layout {
        Layout::column().with(&self.style)
    }

    fn paint(&self, state: &mut ContextAreaState, ctx: &mut PaintCtx<'_>) {
        state.menu.paint(ctx);
    }

    fn event(&self, state: &mut ContextAreaState, event: &Event, ctx: &mut EventCtx<'_>) {
        if state.menu.event(event, ctx) {
            return;
        }
        match *event {
            Event::PointerPressed {
                button: PointerButton::Secondary,
                position,
                ..
            } if ctx.bounds().contains(position) => {
                ctx.set_handled();
                state.menu.open((self.items)(), position, ctx);
            }
            Event::Command(Command::ContextMenu) => {
                ctx.set_handled();
                state.menu.open_by_keys((self.items)(), None, ctx);
            }
            _ => {}
        }
    }
}

impl View for ContextArea {
    fn build(mut self) -> Element {
        let children = self.child.take().into_iter().collect();
        Element::with_children(self, children)
    }
}

impl Styled for ContextArea {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl std::fmt::Debug for ContextArea {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContextArea").finish_non_exhaustive()
    }
}

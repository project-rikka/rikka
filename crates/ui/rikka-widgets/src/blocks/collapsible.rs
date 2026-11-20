use std::cell::RefCell;

use rikka_types::{Point, Rect, Size};
use rikka_ui::{Align, Behavior, Element, Layout, PaintCtx, Style, View, Widget, motion};

use crate::blocks::springing::spring_height;

#[must_use]
#[derive(Widget)]
pub struct Collapsible {
    #[debug]
    open: bool,
    #[debug]
    reveals: bool,
    content: RefCell<Option<Element>>,
    #[style]
    style: Style,
}

pub fn collapsible<V: View>(content: Option<V>) -> Collapsible {
    Collapsible {
        open: content.is_some(),
        reveals: false,
        content: RefCell::new(content.map(View::build)),
        style: Style::default(),
    }
}

impl Collapsible {
    pub fn reveals_on_open(mut self) -> Self {
        self.reveals = true;
        self
    }
}

#[derive(Debug, Default)]
pub struct CollapsibleState {
    shown: Option<f32>,
    full: f32,
    sliding: bool,
    holds: bool,
}

impl Widget for Collapsible {
    type State = CollapsibleState;

    fn layout(&self) -> Layout {
        Layout::row().align_items(Align::Start).with(&self.style)
    }

    fn settled_height(&self, state: &CollapsibleState) -> Option<f32> {
        state.shown
    }

    fn build(&self, state: &mut CollapsibleState) -> Vec<Element> {
        state.sliding |= self.open != state.holds;
        state.holds |= self.open;
        if !state.holds {
            return Vec::new();
        }
        vec![
            CollapsedContent {
                content: RefCell::new(self.content.borrow_mut().take()),
                inert: !self.open,
            }
            .build(),
        ]
    }

    fn paint(&self, state: &mut CollapsibleState, ctx: &mut PaintCtx<'_>) {
        if let Some(child) = ctx.child_rect(0) {
            state.full = child.height();
        }
        let target = match self.open {
            true => state.full,
            false => 0.0,
        };
        let opening = self.open && state.sliding && state.shown.is_some();
        let shown = spring_height(ctx, &mut state.shown, target, motion::GLIDE);
        if self.open && ctx.snap(ctx.bounds().height()) == ctx.snap(target) {
            state.sliding = false;
        }
        if self.reveals && opening {
            let bounds = ctx.bounds();
            ctx.reveal_with_parent_start(Rect::from_origin_size(
                bounds.origin,
                Size::new(bounds.width(), state.full),
            ));
        }
        if !self.open && state.holds && shown <= 0.0 {
            state.holds = false;
            ctx.request_build();
        }
        ctx.set_children_clip(ctx.bounds());
    }

    fn children_offset(&self, state: &CollapsibleState, bounds: Rect, _content: Size) -> Point {
        match state.sliding {
            true => Point::new(0.0, (bounds.height() - state.full).min(0.0)),
            false => Point::ZERO,
        }
    }
}

#[derive(Widget)]
struct CollapsedContent {
    content: RefCell<Option<Element>>,
    #[debug]
    inert: bool,
}

impl Widget for CollapsedContent {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().grow(1.0)
    }

    fn keeps_children(&self) -> bool {
        self.content.borrow().is_none()
    }

    fn build(&self, (): &mut ()) -> Vec<Element> {
        self.content.borrow_mut().take().into_iter().collect()
    }

    fn behavior(&self, (): &()) -> Behavior {
        Behavior::INERT.when(self.inert)
    }
}

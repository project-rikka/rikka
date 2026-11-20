use rikka_types::Rect;
use rikka_ui::text::{Label, TextStyled, text};
use rikka_ui::{Element, Inset, Layout, Length, PaintCtx, Quad, Space, Styled, View, Widget};

use crate::blocks::container::container;
use crate::blocks::flex::column_of;
use crate::blocks::scrollable::scrollable;
use crate::navigation::page_stack::page_stack;

const SIDEBAR: f32 = 216.0;
const HAIRLINE: f32 = 1.0;
const NAV_PADDING: Inset = Inset::exact(12.0, 8.0, 12.0, 8.0);
const PANE: f32 = 28.0;

#[must_use]
pub struct SplitView {
    nav: Option<Element>,
    body: Option<Element>,
    heading: Option<Label>,
    at: Option<usize>,
}

pub fn split_view(nav: impl View, body: impl View) -> SplitView {
    SplitView {
        nav: Some(nav.build()),
        body: Some(body.build()),
        heading: None,
        at: None,
    }
}

impl SplitView {
    pub fn heading(mut self, heading: impl Into<Label>) -> Self {
        self.heading = Some(heading.into());
        self
    }

    pub fn shows(mut self, at: usize) -> Self {
        self.at = Some(at);
        self
    }
}

fn hairline(bounds: Rect) -> Rect {
    Rect::new(bounds.x() + SIDEBAR, bounds.y(), HAIRLINE, bounds.height())
}

impl Widget for SplitView {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row()
            .size(Length::FILL, Length::FILL)
            .gap_exact(HAIRLINE, 0.0)
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        let rule = hairline(ctx.bounds());
        let outline = ctx.theme().palette().outline;
        ctx.draw_quad(Quad::new(rule).fill(outline));
    }
}

impl std::fmt::Debug for SplitView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SplitView")
            .field("heading", &self.heading)
            .finish_non_exhaustive()
    }
}

impl View for SplitView {
    fn build(mut self) -> Element {
        let nav = self.nav.take().expect("navigation already taken");
        let body = self.body.take().expect("body already taken");
        let mut stacked = Vec::with_capacity(2);
        if let Some(heading) = self.heading.take() {
            stacked.push(text(heading).title().bold().build());
        }
        stacked.push(body);

        let pane = scrollable(
            column_of(stacked)
                .gap(Space::LOOSE)
                .padding(Inset::all(PANE)),
        )
        .grow(1.0)
        .height(Length::FILL);
        let pane: Element = match self.at {
            Some(at) => page_stack(at, pane).grow(1.0).height(Length::FILL).build(),
            None => pane.build(),
        };

        let children = vec![
            container(nav.fill())
                .padding(NAV_PADDING)
                .width(Length::px(SIDEBAR))
                .height(Length::FILL)
                .build(),
            pane,
        ];
        Element::with_children(self, children)
    }
}

use std::rc::Rc;

use rikka_types::{Point, Rect, Size};
use rikka_ui::{Behavior, Element, Layout, Length, PaintCtx, Styled, View, Widget};

const UNSCALED: f32 = 1e-4;

type Placing = Rc<dyn Fn(Rect, &mut PaintCtx<'_>) -> Rect>;

struct Framed {
    natural: Size,
    place: Placing,
}

#[derive(Debug, Default)]
struct FramedState {
    shift: Point,
}

#[must_use]
pub fn framed(
    content: impl View,
    natural: Size,
    place: impl Fn(Rect, &mut PaintCtx<'_>) -> Rect + 'static,
) -> Element {
    let content = content
        .build()
        .width(Length::px(natural.width))
        .height(Length::px(natural.height));
    Element::with_children(
        Framed {
            natural,
            place: Rc::new(place),
        },
        vec![content],
    )
}

pub fn fit(ctx: &mut PaintCtx<'_>, natural: Size, into: Rect) -> Point {
    let factor =
        (into.width() / natural.width.max(1.0)).min(into.height() / natural.height.max(1.0));
    let placed = Rect::from_center(
        into.center(),
        Size::new(natural.width * factor, natural.height * factor),
    );
    if (1.0 - factor).abs() >= UNSCALED {
        ctx.set_subtree_scale(factor, placed.origin);
    }
    placed.origin - ctx.bounds().origin
}

impl Widget for Framed {
    type State = FramedState;

    fn layout(&self) -> Layout {
        Layout::column()
            .size(Length::FILL, Length::FILL)
            .placed_at(Point::ZERO)
    }

    fn behavior(&self, _state: &FramedState) -> Behavior {
        Behavior::INERT
    }

    fn paint(&self, state: &mut FramedState, ctx: &mut PaintCtx<'_>) {
        let target = (self.place)(ctx.bounds(), ctx);
        state.shift = fit(ctx, self.natural, target);
    }

    fn children_offset(&self, state: &FramedState, _bounds: Rect, _content: Size) -> Point {
        state.shift
    }
}

impl std::fmt::Debug for Framed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Framed")
            .field("natural", &self.natural)
            .finish_non_exhaustive()
    }
}

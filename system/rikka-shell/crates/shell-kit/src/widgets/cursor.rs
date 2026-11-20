use rikka_types::{Point, Rect};
use rikka_ui::{Length, Styled, View};
use rikka_widgets::{column, row, spacer};

pub const CURSOR: &str = "widgets.cursor";

pub fn cursor(at: Point) -> impl View {
    mark(Rect::new(at.x, at.y, 0.0, 0.0))
}

pub fn mark(rect: Rect) -> impl View {
    column! {
        spacer().height(Length::px(rect.y().max(0.0))).grow(0.0);
        row! {
            spacer().width(Length::px(rect.x().max(0.0))).grow(0.0);
            spacer()
                .width(rect.width().max(0.0))
                .height(rect.height().max(0.0))
                .grow(0.0)
                .build()
                .keyed(CURSOR)
        }
    }
}

pub(crate) fn whole() -> impl View {
    spacer()
        .width(Length::FILL)
        .height(Length::FILL)
        .build()
        .keyed(CURSOR)
}

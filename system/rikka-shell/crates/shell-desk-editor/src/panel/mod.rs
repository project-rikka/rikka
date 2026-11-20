mod carry;
mod gallery;
mod header;
mod inspector;

use rikka_theme::Corner;
use rikka_ui::{
    Edge, Element, Inset, Length, Motion, Property, Space, Styled, Transition, View, is_exiting,
    offset, palette, slide,
};
use rikka_widgets::{column, container};

use crate::EXIT_DURATION;
use crate::editor_canvas::frame::{EDGE, PANEL_WIDTH};
use crate::stage::Editor;

const PADDING: f32 = 24.0;
const BORDER: f32 = 1.0;

pub(crate) fn view(editor: &Editor) -> Element {
    let rect = editor.panel();
    let card = (!is_exiting()).then(|| {
        let body = match editor.selected() {
            Some(widget) => inspector::view(editor, &widget).any(),
            None => gallery::view(editor).any(),
        };
        let palette = palette();
        container(
            column! {
                header::header(editor);
                body;
                header::footer(editor)
            }
            .gap(Space::WIDE)
            .fill(),
        )
        .padding(PADDING)
        .background(palette.surface)
        .border(BORDER, palette.outline)
        .radius(Corner::Window)
        .glass()
        .width(Length::px(PANEL_WIDTH))
        .fill_height()
        .transition(beyond_edge())
    });
    container(column! { card }.fill_height())
        .padding(Inset::edges(0.0, EDGE, 0.0, 0.0))
        .width(Length::px(rect.width()))
        .height(Length::px(rect.height()))
        .position(rect.min_x(), rect.min_y())
        .build()
}

fn beyond_edge() -> Transition {
    (slide(Edge::Trailing) + offset(EDGE, 0.0))
        .exit(Motion::exit(EXIT_DURATION).curve(Property::Spatial))
}

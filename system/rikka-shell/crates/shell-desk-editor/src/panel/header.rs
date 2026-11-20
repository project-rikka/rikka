use rikka_ui::{Align, Length, MaterialSymbol, Space, Styled, TextStyled, View, text, tr};
use rikka_widgets::{Button, ButtonVariant, ControlSize, button, icon, row, spacer};

use crate::stage::Editor;

const GLYPH: f32 = 18.0;

pub(super) fn header(editor: &Editor) -> impl View {
    let history = editor.editing().history();
    row! {
        text(tr!("shell.desk.editor.title")).title().no_wrap().shrink(1.0);
        spacer().grow(1.0);
        tool(MaterialSymbol::UNDO, history.can_undo())
            .on_press(Editor::undo);
        tool(MaterialSymbol::REDO, history.can_redo())
            .on_press(Editor::redo);
        button(text(tr!("shell.desk.editor.done")))
            .variant(ButtonVariant::Primary)
            .control_size(ControlSize::Small)
            .on_press(Editor::close)
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .fill_width()
}

pub(super) fn footer(editor: &Editor) -> impl View {
    let empty = editor.is_empty();
    match editor.is_clearing() {
        true => row! {
            spacer().grow(1.0);
            button(text(tr!("shell.desk.editor.cancel")))
                .variant(ButtonVariant::Ghost)
                .control_size(ControlSize::Small)
                .on_press(|editor: &mut Editor| editor.ask_clearing(false));
            button(text(tr!("shell.desk.editor.clear-yes")))
                .variant(ButtonVariant::Danger)
                .control_size(ControlSize::Small)
                .on_press(Editor::clear)
        }
        .gap(Space::TIGHT)
        .fill_width()
        .any(),
        false => button(
            row! {
                icon(MaterialSymbol::DELETE_SWEEP).extent(Length::px(GLYPH));
                text(tr!("shell.desk.editor.clear"))
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center),
        )
        .variant(ButtonVariant::Ghost)
        .control_size(ControlSize::Small)
        .enabled(!empty && editor.editable())
        .on_press(|editor: &mut Editor| editor.ask_clearing(true))
        .any(),
    }
}

fn tool(symbol: MaterialSymbol, enabled: bool) -> Button {
    button(icon(symbol).extent(Length::px(GLYPH)))
        .variant(ButtonVariant::Ghost)
        .control_size(ControlSize::Small)
        .icon_only()
        .enabled(enabled)
}

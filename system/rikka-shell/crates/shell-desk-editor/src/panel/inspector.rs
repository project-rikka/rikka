use desk_layout::Widget;
use rikka_ui::{
    Align, Label, Length, MaterialSymbol, Space, Styled, TextStyled, View, palette, text, tr,
};
use rikka_widgets::{ButtonVariant, ControlSize, button, column, icon, row, scrollable, segmented};
use shell_desk::widgets;

use crate::sizes;
use crate::stage::Editor;

const KIND_GLYPH: f32 = 22.0;
const ACTION_GLYPH: f32 = 18.0;

pub(super) fn view(editor: &Editor, widget: &Widget) -> impl View {
    let about = widgets::about(widget.kind());
    let id = widget.id;
    let steps = widget.kind().sizes();
    let at = steps
        .iter()
        .position(|size| *size == widget.size)
        .unwrap_or_default();
    column! {
        row! {
            button(icon(MaterialSymbol::ARROW_BACK).extent(Length::px(ACTION_GLYPH)))
                .variant(ButtonVariant::Ghost)
                .control_size(ControlSize::Small)
                .icon_only()
                .on_press(|editor: &mut Editor| editor.select(None));
            icon(about.symbol)
                .extent(Length::px(KIND_GLYPH))
                .color(palette().accent);
            text(about.name).title().no_wrap().shrink(1.0)
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
        text(tr!("shell.desk.editor.size-label")).caption().muted();
        segmented(at)
            .segments(steps.iter().map(|size| sizes::named(*size)))
            .control_size(ControlSize::Small)
            .fill_width()
            .on_select(move |editor: &mut Editor, at: usize| {
                if let Some(size) = steps.get(at) {
                    editor.resize(id, *size);
                }
            });
        scrollable(widgets::options(widget, editor.live(), editor.restyler(id)).fill_width())
            .fill_width()
            .grow(1.0);
        row! {
            button(labelled(MaterialSymbol::CONTENT_COPY, tr!("shell.desk.editor.duplicate")))
                .variant(ButtonVariant::Secondary)
                .on_press(move |editor: &mut Editor| editor.duplicate(id))
                .grow(1.0);
            button(labelled(MaterialSymbol::DELETE, tr!("shell.desk.editor.remove")))
                .variant(ButtonVariant::Danger)
                .on_press(move |editor: &mut Editor| editor.remove(id))
                .grow(1.0)
        }
        .gap(Space::SNUG)
        .fill_width()
    }
    .gap(Space::SNUG)
    .fill_width()
    .grow(1.0)
}

fn labelled(symbol: MaterialSymbol, label: Label) -> impl View {
    row! {
        icon(symbol).extent(Length::px(ACTION_GLYPH));
        text(label)
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
}

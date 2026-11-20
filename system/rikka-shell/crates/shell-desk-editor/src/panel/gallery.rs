use desk_layout::{Content, Kind, Spot, Widget, WidgetId};
use rikka_types::Size;
use rikka_ui::scroll::bar::LANE;
use rikka_ui::{
    Align, Element, Length, MaterialSymbol, Space, Styled, TextStyled, View, palette, text, tr,
};
use rikka_widgets::{
    ButtonVariant, ControlSize, button, column, column_of, container, icon, row, row_of,
    scrollable, text_field,
};
use shell_desk::widgets::{self, Mode};
use shell_kit::{framed, nothing};

use crate::editor_canvas::frame::PANEL_WIDTH;
use crate::panel::PADDING;
use crate::panel::carry::carry;
use crate::sizes;
use crate::stage::Editor;

const SEARCH_GLYPH: f32 = 18.0;
const SEARCH_HEIGHT: f32 = 38.0;
const SAMPLE: WidgetId = WidgetId(0);
const ROOM: f32 = PANEL_WIDTH - PADDING * 2.0 - LANE;

pub(super) fn view(editor: &Editor) -> impl View {
    let search = editor.search();
    let shown: Vec<Kind> = Kind::ALL
        .into_iter()
        .filter(|kind| matches(*kind, search))
        .collect();
    let listing: Element = match shown.is_empty() {
        true => nothing(
            MaterialSymbol::SEARCH_OFF,
            tr!("shell.desk.editor.no-match"),
        )
        .build(),
        false => scrollable(
            column_of(
                shown
                    .into_iter()
                    .map(|kind| entry(editor, kind).build().keyed(kind as u64)),
            )
            .gap(Space::WIDE)
            .fill_width(),
        )
        .fill_width()
        .grow(1.0)
        .build(),
    };
    column! {
        finder(editor);
        listing
    }
    .gap(Space::SNUG)
    .fill_width()
    .grow(1.0)
}

fn finder(editor: &Editor) -> impl View {
    row! {
        icon(MaterialSymbol::SEARCH)
            .extent(Length::px(SEARCH_GLYPH))
            .color(palette().text_muted);
        text_field(editor.query())
            .placeholder(tr!("shell.desk.editor.search"))
            .plain(true)
            .on_change(Editor::typed)
            .grow(1.0)
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .height(SEARCH_HEIGHT)
    .fill_width()
}

fn entry(editor: &Editor, kind: Kind) -> impl View {
    let name = widgets::about(kind).name;
    let sample = Widget::new(SAMPLE, Content::new(kind), Spot::CENTER);
    let preview = shrunk(
        widgets::widget(&sample, editor.live(), Mode::Shown),
        sample.extent(),
    );
    column! {
        carry(kind, editor.session().clone(), preview);
        row! {
            text(name)
                .label()
                .medium()
                .no_wrap()
                .grow(1.0)
                .shrink(1.0);
            chips(kind)
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width()
    }
    .gap(Space::SNUG)
    .fill_width()
}

fn shrunk(preview: Element, natural: Size) -> Element {
    let factor = (ROOM / natural.width).min(1.0);
    container(framed(preview, natural, |bounds, _| bounds))
        .width(Length::px(natural.width * factor))
        .height(Length::px(natural.height * factor))
        .build()
}

fn chips(kind: Kind) -> impl View {
    row_of(kind.sizes().iter().copied().map(move |size| {
        button(text(sizes::short(size)).caption())
            .variant(ButtonVariant::Secondary)
            .control_size(ControlSize::Small)
            .focusless()
            .on_press(move |editor: &mut Editor| editor.add(kind, size))
    }))
    .gap(Space::HAIR)
    .shrink(0.0)
}

fn matches(kind: Kind, search: &str) -> bool {
    if search.is_empty() {
        return true;
    }
    widgets::about(kind)
        .name
        .as_str()
        .to_lowercase()
        .contains(search)
}

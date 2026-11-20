use std::cell::Cell;
use std::rc::Rc;

use rikka_ui::{Axis, Element, Length, Space, TextDocument};
use rikka_widgets::{self as widgets, Slot, Unsaved};

use crate::abi::{enter, violation};
use crate::api::image::Image;
use crate::ffi;
use crate::handlers::Callback;
use crate::values::{finite, fraction, raw_color, selection, symbol};
use crate::view::View;
use crate::view::menu::{MenuEntry, Role};
use crate::view::node::{Built, Kind, Stack};
use crate::window::Model;

fn label(content: &str) -> rikka_ui::Text {
    rikka_ui::text::text(content.to_owned())
}

pub(crate) fn view_palette_color(_: &mut View, role: ffi::ColorRole) -> ffi::Color {
    let palette = rikka_ui::palette();
    raw_color(match role {
        ffi::ColorRole::Background => palette.background,
        ffi::ColorRole::Surface => palette.surface,
        ffi::ColorRole::SurfaceVariant => palette.surface_variant,
        ffi::ColorRole::Text => palette.text,
        ffi::ColorRole::TextMuted => palette.text_muted,
        ffi::ColorRole::Accent => palette.accent,
        ffi::ColorRole::OnAccent => palette.on_accent,
        ffi::ColorRole::Selection => palette.selection,
        ffi::ColorRole::OnSelection => palette.on_selection,
        ffi::ColorRole::Danger => palette.danger,
        ffi::ColorRole::OnDanger => palette.on_danger,
        ffi::ColorRole::Warning => palette.warning,
        ffi::ColorRole::OnWarning => palette.on_warning,
        ffi::ColorRole::Success => palette.success,
        ffi::ColorRole::OnSuccess => palette.on_success,
        ffi::ColorRole::Outline => palette.outline,
        ffi::ColorRole::Overlay => palette.overlay,
    })
}

pub(crate) fn view_is_dark(_: &mut View) -> bool {
    rikka_ui::theme().is_dark()
}

fn columns(columns: u32) -> usize {
    if columns == 0 {
        violation("a grid needs at least 1 column");
    }
    columns as usize
}

pub(crate) fn column_begin(view: &mut View) {
    view.begin(Kind::Flex(Built::ready(widgets::column_of(
        Vec::<Element>::new(),
    ))));
}

pub(crate) fn row_begin(view: &mut View) {
    view.begin(Kind::Flex(Built::ready(widgets::row_of(
        Vec::<Element>::new(),
    ))));
}

pub(crate) fn grid_begin(view: &mut View, count: u32) {
    view.begin(Kind::Flex(Built::ready(widgets::grid_of(
        columns(count),
        Vec::<Element>::new(),
    ))));
}

pub(crate) fn box_begin(view: &mut View) {
    view.begin(Kind::Box {
        container: Built::deferred(),
        stack: Stack::new(Space::NONE, 0.0),
    });
}

pub(crate) fn scroll_begin(view: &mut View) {
    view.begin(Kind::Scroll { follow: false });
}

pub(crate) fn page_begin(view: &mut View) {
    view.begin(Kind::Page);
}

pub(crate) fn section_begin(view: &mut View, heading: &str) {
    view.begin(Kind::Section {
        heading: heading.to_owned(),
        stack: Stack::new(Space::SNUG, 0.0),
    });
}

pub(crate) fn card_begin(view: &mut View) {
    view.begin(Kind::Card {
        stack: Stack::new(Space::SNUG, 0.0),
    });
}

pub(crate) fn field_begin(view: &mut View, text: &str) {
    view.begin(Kind::Field {
        label: text.to_owned(),
    });
}

pub(crate) fn custom_button_begin(view: &mut View) {
    view.begin(Kind::ButtonFrame {
        button: Built::deferred(),
        stack: Stack::new(Space::SNUG, 0.0),
    });
}

pub(crate) fn split_begin(view: &mut View, axis: ffi::Axis, ratio: f32) {
    let ratio = fraction(f64::from(ratio), "split ratio");
    let axis = match axis {
        ffi::Axis::Horizontal => Axis::Horizontal,
        ffi::Axis::Vertical => Axis::Vertical,
    };
    view.begin(Kind::Split {
        axis,
        ratio,
        resize: None,
    });
}

pub(crate) fn page_stack_begin(view: &mut View, page: usize) {
    view.begin(Kind::PageStack { page });
}

pub(crate) fn sheet_begin(view: &mut View) {
    view.begin(Kind::Sheet {
        stack: Stack::new(Space::NORMAL, 20.0),
        dismiss: None,
    });
}

pub(crate) fn popover_begin(view: &mut View) {
    view.begin(Kind::Popover {
        stack: Stack::new(Space::SNUG, 0.0),
        dismiss: None,
    });
}

pub(crate) fn status_page_begin(view: &mut View, icon: &str, message: &str) {
    view.begin(Kind::StatusPage(Built::ready(widgets::status_page(
        symbol(icon),
        message,
    ))));
}

pub(crate) fn context_menu_begin(view: &mut View) {
    view.begin_context_menu();
}

pub(crate) fn popup_menu_begin(view: &mut View) {
    view.begin(Kind::PopupMenu { dismiss: None });
}

pub(crate) fn menu_bar_begin(view: &mut View) {
    view.begin(Kind::MenuBar(Built::ready(widgets::menu_bar())));
}

pub(crate) fn menu_begin(view: &mut View, title: &str) {
    view.begin(Kind::Menu {
        title: title.to_owned(),
    });
}

pub(crate) fn submenu_begin(view: &mut View, text: &str) {
    view.begin(Kind::Submenu(MenuEntry::new(Role::Submenu, text)));
}

pub(crate) fn segmented_begin(view: &mut View, selected: usize) {
    view.begin(Kind::Segmented(Built::ready(widgets::segmented(selected))));
}

pub(crate) fn combo_begin(view: &mut View, selected: usize) {
    view.begin(Kind::Combo(Built::ready(widgets::combo(selection(
        selected,
    )))));
}

pub(crate) fn tabs_begin(view: &mut View, selected: usize) {
    view.begin(Kind::Tabs(Built::ready(widgets::tabs(selected))));
}

pub(crate) fn nav_list_begin(view: &mut View, selected: usize) {
    view.begin(Kind::NavList(Built::ready(widgets::nav_list(selected))));
}

pub(crate) fn list_begin(view: &mut View) {
    view.begin(Kind::List(Built::deferred()));
}

pub(crate) fn end(view: &mut View) {
    view.end();
}

pub(crate) fn memo(view: &mut View, digest: u64, content: Callback<ffi::ViewFn>) {
    let memoized = widgets::lazy(digest, move || {
        let _entry = enter("rk_memo content");
        let mut declared = View::fragment();
        content.call(&mut declared);
        declared.finish_fragment()
    });
    view.leaf(Kind::Memo(Built::ready(memoized)));
}

pub(crate) fn text(view: &mut View, content: &str) {
    view.leaf(Kind::Text(Built::ready(label(content))));
}

pub(crate) fn icon(view: &mut View, name: &str) {
    view.leaf(Kind::Icon(Built::ready(widgets::icon(symbol(name)))));
}

pub(crate) fn picture(view: &mut View, picture: widgets::Image) {
    view.leaf(Kind::Picture(Built::ready(picture)));
    let style = &mut view.current_element().common.style;
    style.min_width = Some(Length::px(0.0));
    style.min_height = Some(Length::px(0.0));
}

pub(crate) fn image_view(view: &mut View, image: &Image) {
    picture(view, widgets::image(image.source()));
}

pub(crate) fn spacer(view: &mut View) {
    view.leaf(Kind::Spacer);
}

pub(crate) fn divider(view: &mut View) {
    view.leaf(Kind::Divider { vertical: false });
}

pub(crate) fn vertical_divider(view: &mut View) {
    view.leaf(Kind::Divider { vertical: true });
}

pub(crate) fn button(view: &mut View, text: &str) {
    view.leaf(Kind::Button(Built::ready(widgets::button(label(text)))));
}

pub(crate) fn icon_button(view: &mut View, name: &str) {
    view.leaf(Kind::Button(Built::ready(
        widgets::button(widgets::icon(symbol(name)))
            .icon_only()
            .ghost(),
    )));
}

pub(crate) fn checkbox(view: &mut View, state: ffi::CheckState, text: &str) {
    let mut built = widgets::checkbox(check_state(state));
    if !text.is_empty() {
        built = built.label(label(text));
    }
    view.leaf(Kind::Checkbox(Built::ready(built)));
}

pub(crate) fn toggle(view: &mut View, on: bool) {
    view.leaf(Kind::Toggle(Built::ready(widgets::switch(on))));
}

pub(crate) fn radio(view: &mut View, selected: bool, text: &str) {
    let mut built = widgets::radio(selected);
    if !text.is_empty() {
        built = built.label(label(text));
    }
    view.leaf(Kind::Radio(Built::ready(built)));
}

pub(crate) fn slider(view: &mut View, value: f64) {
    let value = finite(value, "slider value") as f32;
    view.leaf(Kind::Slider(Built::ready(widgets::slider(value))));
}

pub(crate) fn number_field(view: &mut View, value: f64) {
    let value = finite(value, "number field value") as f32;
    view.leaf(Kind::NumberField(Built::ready(widgets::number_field(
        value,
    ))));
}

pub(crate) fn progress(view: &mut View, value: f64) {
    view.leaf(Kind::Progress(Built::ready(widgets::progress(fraction(
        value, "progress",
    )))));
}

pub(crate) fn progress_indeterminate(view: &mut View) {
    view.leaf(Kind::Progress(Built::ready(
        widgets::progress_indeterminate(),
    )));
}

pub(crate) fn spinner(view: &mut View) {
    view.leaf(Kind::Spinner(Built::ready(widgets::spinner())));
}

pub(crate) fn gauge(view: &mut View, value: f64) {
    view.leaf(Kind::Gauge(Built::ready(widgets::gauge(fraction(
        value, "gauge",
    )))));
}

pub(crate) fn text_field(view: &mut View, document: &TextDocument) {
    view.leaf(Kind::TextField(Built::ready(widgets::text_field(document))));
}

pub(crate) fn text_editor(view: &mut View, document: &TextDocument) {
    view.leaf(Kind::TextEditor(Built::ready(widgets::text_editor(
        document,
    ))));
}

pub(crate) fn banner(view: &mut View, message: &str) {
    view.leaf(Kind::Banner(Box::new(Built::ready(widgets::banner(
        message.to_owned(),
    )))));
}

pub(crate) fn unsaved_prompt(
    view: &mut View,
    name: &str,
    saving: bool,
    callback: Callback<ffi::UnsavedFn>,
) {
    let prompt =
        widgets::unsaved_prompt(name, saving, move |model: &mut Model, answer: Unsaved| {
            let answer = match answer {
                Unsaved::Save => ffi::UnsavedAnswer::Save,
                Unsaved::Discard => ffi::UnsavedAnswer::Discard,
                Unsaved::Cancel => ffi::UnsavedAnswer::Cancel,
            };
            callback.call(&mut model.context(), answer);
        });
    view.leaf(Kind::Unsaved(Box::new(prompt)));
}

pub(crate) fn lazy_list(view: &mut View, count: usize, item: Callback<ffi::ItemFn>) {
    let selected: Rc<Cell<Option<usize>>> = Rc::default();
    let shown = Rc::clone(&selected);
    let rows = widgets::list_of(count, move |index| {
        let _entry = enter("rk_lazy_list item");
        let mut declared = View::list_item();
        item.call(&mut declared, index, shown.get() == Some(index));
        declared.finish_list_item()
    });
    view.leaf(Kind::LazyList {
        list: Built::ready(rows),
        selected,
    });
}

fn cell(item: Callback<ffi::ItemFn>, entry: &'static str) -> impl Fn(Slot) -> Element + 'static {
    move |slot: Slot| {
        let _entry = enter(entry);
        let mut declared = View::fragment();
        item.call(&mut declared, slot.index, slot.selected);
        declared.finish_fragment()
    }
}

pub(crate) fn lazy_column(view: &mut View, count: usize, item: Callback<ffi::ItemFn>) {
    view.leaf(Kind::Collection(Built::ready(widgets::lazy_column(
        count,
        cell(item, "rk_lazy_column item"),
    ))));
}

pub(crate) fn lazy_grid(view: &mut View, count: usize, grid: u32, item: Callback<ffi::ItemFn>) {
    view.leaf(Kind::Collection(Built::ready(widgets::lazy_grid(
        columns(grid),
        count,
        cell(item, "rk_lazy_grid item"),
    ))));
}

pub(crate) fn check_state(state: ffi::CheckState) -> widgets::CheckState {
    match state {
        ffi::CheckState::Unchecked => widgets::CheckState::Unchecked,
        ffi::CheckState::Checked => widgets::CheckState::Checked,
        ffi::CheckState::Indeterminate => widgets::CheckState::Indeterminate,
    }
}

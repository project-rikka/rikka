use rikka_session::replay::{BufferedSubject, Kind};
use rikka_types::{Key, Modifiers};
use rikka_ui::{
    Align, Element, Inset, Justify, Label, Length, MaterialSymbol, Space, Styled, TextStyled, View,
    palette, text, tr,
};
use rikka_widgets::{ControlSize, column, icon, list, list_item, scrollable};
use shell_kit::{EDITOR_EDGE, EDITOR_ICON, editor_error, editor_loading, editor_window};

use crate::replay::chooser::{Chooser, Listing};

const LEAST: f32 = 160.0;
const MOST: f32 = 420.0;

pub(crate) fn view(chooser: &Chooser) -> impl View {
    editor_window(
        chooser.spot(),
        MaterialSymbol::REPLAY,
        tr!("shell.replay.title"),
        Chooser::close,
    )
    .on_key(on_key)
    .body(body(chooser))
}

fn on_key(chooser: &mut Chooser, key: Key, _: Modifiers) -> bool {
    match key {
        Key::Escape => chooser.close(),
        Key::ArrowUp => chooser.step(-1),
        Key::ArrowDown => chooser.step(1),
        Key::Enter => chooser.choose_selected(),
        _ => return false,
    }
    true
}

fn body(chooser: &Chooser) -> Element {
    let buffer = match chooser.listing() {
        Listing::Asking => return editor_loading(LEAST).build(),
        Listing::Failed(trouble) => return editor_error(trouble, LEAST).build(),
        Listing::Loaded(buffer) => buffer,
    };
    if !buffer.running {
        return status_message(MaterialSymbol::VIDEOCAM_OFF, tr!("shell.replay.off"));
    }
    if buffer.subjects.is_empty() {
        return status_message(MaterialSymbol::HOURGLASS_EMPTY, tr!("shell.replay.empty"));
    }
    let drafting = chooser.drafting();
    let rows = buffer.subjects.iter().map(|row_subject| {
        let busy = drafting == Some(row_subject.id.as_str());
        list_item(title(row_subject))
            .keyed(row_subject.id.clone())
            .icon(symbol(row_subject))
            .tint(palette().accent)
            .secondary(detail(row_subject))
            .trailing(clock(row_subject.seconds))
            .busy(busy)
            .enabled(drafting.is_none())
    });
    let subjects: Vec<String> = buffer
        .subjects
        .iter()
        .map(|row_subject| row_subject.id.clone())
        .collect();
    let chosen = list(rows)
        .selected(chooser.selected())
        .control_size(ControlSize::Large)
        .activates_on_press(true)
        .on_activate(move |chooser: &mut Chooser, index: usize| {
            if let Some(subject) = subjects.get(index) {
                chooser.choose(subject.clone());
            }
        });
    let trouble = chooser
        .trouble()
        .map(|trouble| text(trouble.to_string()).caption().muted().build())
        .unwrap_or_else(Element::empty);
    column! {
        scrollable(chosen).max_height(MOST);
        trouble
    }
    .gap(Space::TIGHT)
    .padding(Inset::xy(EDITOR_EDGE, Space::NORMAL.px()))
    .fill_width()
    .build()
}

fn status_message(symbol: MaterialSymbol, shown_text: Label) -> Element {
    column! {
        icon(symbol)
            .extent(Length::px(EDITOR_ICON))
            .color(palette().text_muted);
        text(shown_text).label().muted()
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .justify(Justify::Center)
    .height(LEAST)
    .fill_width()
    .build()
}

fn symbol(row_subject: &BufferedSubject) -> MaterialSymbol {
    match row_subject.kind {
        Kind::Display { .. } => MaterialSymbol::MONITOR,
        Kind::Window { .. } => MaterialSymbol::DESKTOP_WINDOWS,
    }
}

fn title(row_subject: &BufferedSubject) -> Label {
    match &row_subject.kind {
        Kind::Display { name, description } if description.trim().is_empty() => {
            Label::from(name.as_str())
        }
        Kind::Display { description, .. } => Label::from(description.as_str()),
        Kind::Window { title, app_id } if title.trim().is_empty() => Label::from(app_id.as_str()),
        Kind::Window { title, .. } => Label::from(title.as_str()),
    }
}

fn detail(row_subject: &BufferedSubject) -> Label {
    let kind = match &row_subject.kind {
        Kind::Display { name, .. } => tr!("shell.replay.display", name = name.as_str()),
        Kind::Window { app_id, .. } => Label::from(app_id.as_str()),
    };
    match row_subject.ended {
        true => tr!("shell.replay.ended", kind = &kind.to_string()),
        false => kind,
    }
}

fn clock(seconds: f64) -> Label {
    let whole = std::time::Duration::from_secs(seconds.max(0.0).round() as u64);
    Label::from(rikka_format::format_duration(whole))
}

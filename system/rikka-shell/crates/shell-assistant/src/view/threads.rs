use rikka_format::RelativeTime;
use rikka_theme::Corner;
use rikka_thread::Summary;
use rikka_ui::{
    Align, Element, Inset, Label, Length, MaterialSymbol, Space, Styled, TextStyled, View, palette,
    text, tr,
};
use rikka_widgets::{
    ControlSize, button, column, column_of, container, icon, row, scrollable, spacer,
};

use crate::assistance::listed;
use crate::panel::Panel;

const GLYPH: f32 = 15.0;
const EDGE: f32 = 12.0;

pub(super) fn view(panel: &Panel) -> Element {
    let shown = panel.shown();
    let listed = listed(shown);
    if listed.is_empty() {
        return nothing().build();
    }

    let open = shown.thread.id().to_owned();
    let rows = listed
        .iter()
        .map(|summary| entry(summary, summary.id() == open));

    scrollable(
        column_of(rows)
            .gap(Space::HAIR)
            .padding(Inset::xy(EDGE, 0.0))
            .fill_width(),
    )
    .grow(1.0)
    .fill_width()
    .build()
}

fn entry(summary: &Summary, open: bool) -> impl View {
    let palette = palette();
    let ink = match open {
        true => palette.accent,
        false => palette.text,
    };
    let id = summary.id().to_owned();
    let discarded = id.clone();

    let message = column! {
        text(titled(summary))
            .label()
            .color(ink)
            .no_wrap()
            .fill_width();
        text(relative_time(summary.updated()))
            .caption()
            .muted()
    }
    .gap(Space::HAIR)
    .grow(1.0)
    .fill_width();

    let line = row! {
        message;
        button(
            icon(MaterialSymbol::DELETE)
                .extent(Length::px(GLYPH))
                .color(palette.text_muted)
        )
        .icon_only()
        .ghost()
        .control_size(ControlSize::Small)
        .on_press(move |panel: &mut Panel| panel.discard(&discarded))
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .fill_width();

    button(line)
        .ghost()
        .on_press(move |panel: &mut Panel| panel.open(&id))
        .fill_width()
}

fn titled(summary: &Summary) -> Label {
    match summary.title().trim().is_empty() {
        true => tr!("shell.assistant.untitled"),
        false => Label::from(summary.title()),
    }
}

fn relative_time(at: u64) -> Label {
    #[expect(
        clippy::cast_possible_wrap,
        reason = "a count of minutes, hours or days, which no conversation has more of than an i64 holds"
    )]
    match rikka_format::time_ago(at) {
        Some(RelativeTime::Minutes(count)) => {
            tr!("shell.assistant.span.minutes", count = count as i64)
        }
        Some(RelativeTime::Hours(count)) => tr!("shell.assistant.span.hours", count = count as i64),
        Some(RelativeTime::Days(count)) => tr!("shell.assistant.span.days", count = count as i64),
        None => tr!("shell.assistant.span.now"),
    }
}

fn nothing() -> impl View {
    container(
        column! {
            spacer();
            text(tr!("shell.assistant.no-threads")).caption().muted()
        }
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::xy(EDGE, EDGE))
    .align_y(Align::Center)
    .grow(1.0)
    .fill_width()
    .radius(Corner::Item)
}

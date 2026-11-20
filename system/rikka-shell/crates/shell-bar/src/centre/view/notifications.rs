use rikka_ui::{Align, Element, Inset, Space, Styled, TextStyled, View, fade, text, tr};
use rikka_widgets::{
    ButtonVariant, ControlSize, button, collapsible, column, column_of, container, dismissible,
    row, scrollable, spacer,
};
use shell_notifications::{HistoryRow, card};

use crate::centre::state::Centre;
use crate::centre::view::tile;

const LIST: f32 = 320.0;

const NOTIFICATIONS: &str = "centre.notifications";

const INSET: f32 = 10.0;

pub(crate) fn view(centre: &Centre) -> impl View {
    let list = centre.notifications();
    let today = jiff::Zoned::now().date();
    let rows: Vec<Element> = list.rows().iter().map(|row| row_for(row, today)).collect();
    let empty = rows.is_empty();
    let body = column! {
        collapsible(empty.then(nothing)).fill_width();
        scrollable(
            column_of(rows)
                .align_items(Align::Stretch)
                .fill_width(),
        )
        .max_height(LIST)
        .build()
        .keyed(NOTIFICATIONS)
    }
    .align_items(Align::Stretch)
    .fill_width();
    column! {
        heading(list.has_standing());
        body
    }
    .gap(Space::SNUG)
    .align_items(Align::Stretch)
    .fill_width()
}

fn heading(any: bool) -> impl View {
    row! {
        tile::caption(tr!("shell.centre.notifications"));
        spacer().grow(1.0);
        if any {
            button(text(tr!("shell.centre.clear-notifications")).caption())
                .variant(ButtonVariant::Secondary)
                .control_size(ControlSize::Small)
                .on_press(Centre::clear_notifications)
                .transition(fade());
        }
    }
    .align_items(Align::Center)
    .fill_width()
}

fn nothing() -> impl View {
    tile::tile(vec![
        container(text(tr!("shell.centre.no-notifications")).caption().muted())
            .height(tile::ROW)
            .fill_width()
            .center()
            .build(),
    ])
}

fn row_for(row: &HistoryRow, today: jiff::civil::Date) -> Element {
    let stored = &row.stored;
    let id = stored.id();
    let card = card::card(
        &stored.notification,
        &stored.look,
        move |centre: &mut Centre| centre.activate(id),
    )
    .sized(tile::ICON, tile::RADIUS, INSET)
    .at(card::posted_at(&stored.look, Some(today)))
    .closed(move |centre: &mut Centre| centre.forget(id))
    .acted(move |centre: &mut Centre, key: String| centre.act(id, &key))
    .followed(move |centre: &mut Centre, at| centre.follow(id, at))
    .build();
    dismissible(
        container(card)
            .padding(Inset::edges(0.0, 0.0, Space::TIGHT, 0.0))
            .fill_width(),
    )
    .arriving(row.arriving)
    .dismissed(row.leaving)
    .on_dismiss(move |centre: &mut Centre| centre.forget(id))
    .fill_width()
    .build()
    .keyed(row.key)
}

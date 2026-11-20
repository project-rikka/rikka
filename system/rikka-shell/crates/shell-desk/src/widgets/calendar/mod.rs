mod day;
mod month;
mod region;

use desk_layout::{Calendar, Size, WidgetId};
use jiff::civil::{Date, Weekday};
use rikka_ui::{
    Align, Element, Justify, Length, MaterialSymbol, Space, Styled, TextAlign, TextRole,
    TextStyled, View, palette, text,
};
use rikka_widgets::{ControlSize, button, column, column_of, icon, row, row_of, spacer};
use shell_feeds::{NameLength, dates, weekday};

use crate::widgets::calendar::day::{Sheet, initials};
use crate::widgets::calendar::region::Week;
use crate::widgets::card::padded;
use crate::widgets::kit::{Header, LINE, display, figures, header};
use crate::widgets::live::Live;
use crate::widgets::sources::Sources;

const SMALL_DAY: f32 = 52.0;
const STRIP_DAY: f32 = 18.0;
const MEDIUM_DAY: f32 = 48.0;
const MEDIUM_SIDE: f32 = 92.0;
const MEDIUM_CELL: f32 = 18.0;
const LARGE_CELL: f32 = 32.0;
const WEEK_COLUMN: f32 = 22.0;
const WEEK_NUMBER: f32 = 10.0;
const TURN: f32 = 18.0;

#[must_use]
pub(crate) fn view(id: WidgetId, calendar: &Calendar, size: Size, sources: &Sources) -> Element {
    let today = sources.feeds.wall.today();
    let week = Week::from_first_day(calendar.first_day);
    match size {
        Size::Small => small(today, week),
        Size::Medium => medium(today, week),
        Size::Large | Size::ExtraLarge => large(id, today, week, sources.page(id)),
    }
}

fn weekday_of(today: Date) -> Header {
    header(None, weekday(today.weekday(), NameLength::Long))
}

fn small(today: Date, week: Week) -> Element {
    let sheet = Sheet {
        today,
        shown: today,
        week,
    };
    let strip = month::week(today, week.first).map(|date| {
        column! {
            text(dates::initial(date.weekday()))
                .text_size(WEEK_NUMBER)
                .semibold()
                .faint()
                .align(TextAlign::Center)
                .no_wrap();
            sheet.mark(date, STRIP_DAY, TextRole::Caption);
        }
        .gap(LINE)
        .align_items(Align::Center)
        .grow(1.0)
        .min_width(0.0)
        .build()
    });
    padded(
        column! {
            weekday_of(today).trailing(dates::week(today));
            display(today.day().to_string(), SMALL_DAY);
            text(dates::month_of_year(today)).label().muted().no_wrap();
            spacer().grow(1.0);
            row_of(strip).fill_width();
        }
        .gap(LINE)
        .fill(),
    )
}

fn medium(today: Date, week: Week) -> Element {
    let sheet = Sheet {
        today,
        shown: today,
        week,
    };
    let rows = month::weeks(today, week.first).into_iter().map(|row| {
        row_of(
            row.iter()
                .map(|date| sheet.cell(*date, MEDIUM_CELL, TextRole::Caption)),
        )
        .align_items(Align::Center)
        .grow(1.0)
        .fill_width()
        .build()
    });
    padded(
        row! {
            column! {
                weekday_of(today);
                display(today.day().to_string(), MEDIUM_DAY);
                text(dates::month(today)).label().muted().no_wrap();
                spacer().grow(1.0);
                figures(dates::week(today)).caption().faint().no_wrap();
            }
            .gap(LINE)
            .width(Length::px(MEDIUM_SIDE))
            .fill_height();
            column_of(std::iter::once(initials(week).fill_width().build()).chain(rows))
                .grow(1.0)
                .min_width(0.0)
                .fill_height();
        }
        .gap(Space::NORMAL)
        .fill(),
    )
}

fn large(id: WidgetId, today: Date, week: Week, page: i32) -> Element {
    let shown = month::paged(today, page);
    let sheet = Sheet { today, shown, week };
    let title = text(dates::month_of_year(shown))
        .subheading()
        .semibold()
        .no_wrap();
    let title = match page != 0 {
        true => title.color(palette().accent),
        false => title,
    };
    let rows = month::weeks(shown, week.first).into_iter().map(|row| {
        let numbered = row
            .iter()
            .find(|date| date.weekday() == Weekday::Thursday)
            .map_or(0, |date| date.iso_week_date().week());
        row! {
            figures(numbered.to_string())
                .text_size(WEEK_NUMBER)
                .faint()
                .align(TextAlign::Center)
                .no_wrap()
                .width(Length::px(WEEK_COLUMN));
            row_of(
                row.iter()
                    .map(|date| sheet.cell(*date, LARGE_CELL, TextRole::Label)),
            )
            .align_items(Align::Center)
            .grow(1.0)
            .min_width(0.0);
        }
        .align_items(Align::Center)
        .grow(1.0)
        .fill_width()
        .build()
    });
    padded(
        column! {
            row! {
                button(title)
                    .ghost()
                    .control_size(ControlSize::Small)
                    .on_press(move |live: &mut Live| live.back_to_today(id));
                spacer().grow(1.0);
                turn(MaterialSymbol::CHEVRON_LEFT, id, -1);
                turn(MaterialSymbol::CHEVRON_RIGHT, id, 1);
            }
            .gap(Space::HAIR)
            .align_items(Align::Center)
            .fill_width();
            row! {
                spacer().width(Length::px(WEEK_COLUMN));
                initials(week).grow(1.0).min_width(0.0);
            }
            .fill_width();
            column_of(rows).grow(1.0).fill_width();
        }
        .gap(Space::TIGHT)
        .justify(Justify::Start)
        .fill(),
    )
}

fn turn(symbol: MaterialSymbol, id: WidgetId, by: i32) -> impl View {
    button(icon(symbol).extent(Length::px(TURN)))
        .icon_only()
        .ghost()
        .control_size(ControlSize::Small)
        .on_press(move |live: &mut Live| live.turn_page(id, by))
}

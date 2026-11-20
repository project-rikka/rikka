use std::sync::Arc;

use music_core::Track;
use music_library::{Order, SongSort, Songs};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::blocks::{Listening, UNAVAILABLE};
use crate::local::arrange::Arrange;
use crate::local::empty::empty;
use crate::local::fresh::Shelf;
use crate::local::title_bar;
use crate::menus::{self, Subject, TrackContext, TrackTarget};
use crate::music::Music;
use crate::player::Player;
use crate::widgets::{bars, high};
use crate::wording::{self, length};

const LINE: f32 = 40.0;
const YEAR: f32 = 48.0;
const FORMAT: f32 = 184.0;
const PLAYS: f32 = 52.0;
const ADDED: f32 = 96.0;
const TIME: f32 = 52.0;
const NOW: f32 = 14.0;

struct Table {
    rows: Arc<[music_library::Track]>,
    tracks: Arc<[Track]>,
}

#[derive(Clone, Copy)]
enum Column {
    Title,
    Artist,
    Album,
    Year,
    Format,
    Plays,
    Added,
    Length,
}

impl Column {
    const ALL: [Column; 8] = [
        Column::Title,
        Column::Artist,
        Column::Album,
        Column::Year,
        Column::Format,
        Column::Plays,
        Column::Added,
        Column::Length,
    ];

    fn sort(self) -> SongSort {
        match self {
            Column::Title => SongSort::Title,
            Column::Artist => SongSort::Artist,
            Column::Album => SongSort::Album,
            Column::Year => SongSort::Year,
            Column::Format => SongSort::Codec,
            Column::Plays => SongSort::Plays,
            Column::Added => SongSort::Added,
            Column::Length => SongSort::Duration,
        }
    }

    fn heading(self) -> Label {
        match self {
            Column::Title => tr!("music.column.title"),
            Column::Artist => tr!("music.column.artist"),
            Column::Album => tr!("music.column.album"),
            Column::Year => tr!("music.column.year"),
            Column::Format => tr!("music.column.format"),
            Column::Plays => tr!("music.column.plays"),
            Column::Added => tr!("music.column.added"),
            Column::Length => tr!("music.column.length"),
        }
    }

    fn trailing(self) -> bool {
        matches!(self, Column::Plays | Column::Length)
    }

    fn sized<V: Styled>(self, cell: V) -> V {
        match self {
            Column::Title => cell.grow(3.0).min_width(0.0),
            Column::Artist | Column::Album => cell.grow(2.0).min_width(0.0),
            Column::Year => cell.width(YEAR),
            Column::Format => cell.width(FORMAT),
            Column::Plays => cell.width(PLAYS),
            Column::Added => cell.width(ADDED),
            Column::Length => cell.width(TIME),
        }
    }
}

pub(super) fn view(music: &Music, shelf: &Shelf, listening: &Listening) -> Element {
    let (sort, order) = music.arrange().songs();
    let scanning = music.scanning();
    let query = Songs::new(sort).order(order);
    let loading = shelf.read_listening(query, move |library| {
        let rows = library.songs(query)?;
        Ok(Table {
            tracks: music_local::tracks(&rows).into(),
            rows: rows.into(),
        })
    });
    let listening = listening.clone();
    crate::pages::fetched(loading, move |table: &Table| {
        if table.rows.is_empty() {
            return empty(scanning);
        }
        table_of(table, sort, order, &listening)
    })
}

fn table_of(table: &Table, sort: SongSort, order: Order, listening: &Listening) -> Element {
    let tracks = Arc::clone(&table.tracks);
    let rows = Arc::clone(&table.rows);
    let (played, shuffled) = (Arc::clone(&tracks), Arc::clone(&tracks));
    let listening = listening.clone();
    let count = rows.len();
    column! {
        title_bar(
            tr!("music.nav.songs-all"),
            wording::songs(count),
            row! {
                button(
                    row! {
                        icon(MaterialSymbol::PLAY_ARROW).filled().extent(Length::px(18.0));
                        text(tr!("music.play")).label().semibold();
                    }
                    .gap(Space::TIGHT)
                    .align_items(Align::Center),
                )
                .on_press(move |player: &mut Player| player.play(&played, 0));
                button(
                    row! {
                        icon(MaterialSymbol::SHUFFLE).extent(Length::px(18.0));
                        text(tr!("music.shuffle")).label();
                    }
                    .gap(Space::TIGHT)
                    .align_items(Align::Center),
                )
                .secondary()
                .on_press(move |player: &mut Player| player.shuffled(&shuffled));
            }
            .gap(Space::SNUG),
        );
        header(sort, order);
        rule();
        lazy_column(count, move |slot| line(&rows, &tracks, slot.index, &listening))
            .estimate(LINE)
            .padding(Inset::edges(Space::TIGHT, Space::LOOSE, Space::WIDE, Space::LOOSE))
            .grow(1.0)
            .fill_width();
    }
    .fill()
    .any()
}

fn header(sort: SongSort, order: Order) -> impl View {
    row_of(Column::ALL.map(|column| {
        let chosen = column.sort() == sort;
        let tint = match chosen {
            true => palette().text,
            false => palette().text_muted,
        };
        let arrow = chosen.then(|| {
            icon(match order {
                Order::Ascending => MaterialSymbol::ARROW_UPWARD,
                Order::Descending => MaterialSymbol::ARROW_DOWNWARD,
            })
            .extent(Length::px(13.0))
            .color(tint)
        });
        let label = text(column.heading())
            .caption()
            .semibold()
            .color(tint)
            .no_wrap()
            .shrink(1.0)
            .min_width(0.0);
        let face = match column.trailing() {
            true => row! { arrow; label; },
            false => row! { label; arrow; },
        };
        let cell = container(
            face.gap(Space::HAIR)
                .align_items(Align::Center)
                .justify(match column.trailing() {
                    true => Justify::End,
                    false => Justify::Start,
                })
                .fill_width(),
        )
        .padding(Inset::xy(Space::TIGHT, Space::TIGHT))
        .radius(Corner::Item)
        .on_press(move |arrange: &mut Arrange| arrange.sort_songs(column.sort()));
        column.sized(cell).build()
    }))
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .padding(Inset::edges(
        Space::NONE,
        Space::WIDE,
        Space::TIGHT,
        Space::WIDE,
    ))
    .fill_width()
}

fn line(
    rows: &Arc<[music_library::Track]>,
    tracks: &Arc<[Track]>,
    index: usize,
    listening: &Listening,
) -> Element {
    let (Some(row), Some(track)) = (rows.get(index), tracks.get(index)) else {
        return Element::empty();
    };
    let now = listening.is_now(track);
    let title_color = match now {
        true => palette().accent,
        false => palette().text,
    };
    let muted = |message: String| text(message).caption().muted().no_wrap();
    let cells = Column::ALL.map(|column| {
        let cell: Element = match column {
            Column::Title => row! {
                now.then(|| bars(listening.playing, NOW));
                text(row.title.clone()).label().medium().color(title_color).no_wrap().min_width(0.0);
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center)
            .any(),
            Column::Artist => muted(row.artist.clone()).any(),
            Column::Album => muted(row.album.title.clone()).any(),
            Column::Year => muted(row.year.map(|year| year.to_string()).unwrap_or_default()).any(),
            Column::Format => row! {
                high(track.fidelity).map(|badge| badge.shrink(0.0));
                muted(track.fidelity.map(wording::fidelity).unwrap_or_default()).min_width(0.0);
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center)
            .any(),
            Column::Plays => muted(row.listening.plays.to_string())
                .align(TextAlign::End)
                .fill_width()
                .any(),
            Column::Added => muted(wording::day(row.added)).any(),
            Column::Length => muted(length(row.duration))
                .align(TextAlign::End)
                .fill_width()
                .any(),
        };
        column.sized(container(cell).padding(Inset::xy(Space::TIGHT, Space::NONE))).build()
    });
    let body = container(
        row_of(cells)
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width(),
    )
    .padding(Inset::xy(Space::SNUG, Space::NONE))
    .align_y(Align::Center)
    .radius(Corner::Item)
    .background(match now {
        true => palette().accent.with_alpha(0.08),
        false => Color::TRANSPARENT,
    })
    .fill();
    let list = Arc::clone(tracks);
    let body = match track.playable {
        true => body
            .on_press(move |player: &mut Player| player.play(&list, index))
            .any(),
        false => body.opacity(UNAVAILABLE).any(),
    };
    let subject = Subject::Track(TrackTarget {
        track: track.clone(),
        context: TrackContext::Listed,
    });
    menus::target(subject, body).height(LINE).fill_width()
}

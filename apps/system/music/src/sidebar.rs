use std::sync::Arc;

use music_blend::Blend;
use music_core::Playlist;
use music_library::Progress;
use rikka_app::prelude::*;
use rikka_widgets::row;

use crate::blocks::MediaLine;
use crate::library::{Library, Load};
use crate::local::scan_line;
use crate::menus::{self, Subject};
use crate::music::Music;
use crate::nav::Place;
use crate::wording::songs;

pub(crate) const SIDEBAR: f32 = 248.0;
const ENTRY: f32 = 52.0;
const HEADING: f32 = 44.0;
const DESTINATION: f32 = 40.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Heading {
    Blends,
    Playlists,
}

#[derive(Clone, Debug)]
enum Line {
    Destination(MaterialSymbol, Label, Place),
    Scanning(Progress),
    Heading(Heading),
    Blend(Arc<Blend>),
    Playlist(Playlist),
    Loading,
    Failed(String),
}

pub(crate) fn view(music: &Music) -> impl View {
    let current = music.nav().current().clone();
    let origin = music.nav().origin().clone();
    let mut lines = destinations(music.browses_local());
    if music.browses_local() {
        lines.push(Line::Scanning(music.scanning()));
    }
    lines.push(Line::Heading(Heading::Blends));
    lines.extend(music.blends().listed().iter().cloned().map(Line::Blend));
    if !music.browses_local() {
        playlists(music, &mut lines);
    }
    let syncing = music.blends().any_syncing();
    let linked = music.has_links();
    lazy_column(lines.len(), move |slot| match &lines[slot.index] {
        Line::Destination(symbol, title, place) => {
            destination(*symbol, title.clone(), place.clone(), &origin).any()
        }
        Line::Scanning(progress) => scan_line(*progress).unwrap_or_else(Element::empty),
        Line::Heading(heading) => heading_line(*heading, syncing, linked),
        Line::Blend(blend) => blend_entry(blend, current == Place::Blend(blend.id().clone())),
        Line::Playlist(playlist) => {
            playlist_entry(playlist, current == Place::Playlist(playlist.id.clone()))
        }
        Line::Loading => container(spinner().extent(Length::px(18.0)))
            .center()
            .height(ENTRY)
            .fill_width()
            .any(),
        Line::Failed(why) => container(text(why.clone()).caption().danger())
            .padding(Inset::xy(Space::ROOMY, Space::TIGHT))
            .fill_width()
            .any(),
    })
    .estimate(ENTRY)
    .padding(Inset::xy(Space::SNUG, Space::SNUG))
    .width(SIDEBAR)
    .fill_height()
}

fn destinations(local: bool) -> Vec<Line> {
    let line = |symbol, title, place| Line::Destination(symbol, title, place);
    let mut lines = vec![
        line(MaterialSymbol::HOME, tr!("music.nav.home"), Place::Home),
        line(
            MaterialSymbol::SEARCH,
            tr!("music.nav.search"),
            Place::Search,
        ),
    ];
    if local {
        lines.extend([
            line(
                MaterialSymbol::ALBUM,
                tr!("music.nav.albums"),
                Place::Albums,
            ),
            line(
                MaterialSymbol::PERSON,
                tr!("music.nav.artists"),
                Place::Artists,
            ),
            line(
                MaterialSymbol::MUSIC_NOTE,
                tr!("music.nav.songs-all"),
                Place::Songs,
            ),
            line(MaterialSymbol::SELL, tr!("music.nav.genres"), Place::Genres),
            line(
                MaterialSymbol::FOLDER,
                tr!("music.nav.folders"),
                Place::Folders,
            ),
        ]);
    }
    lines.push(line(
        MaterialSymbol::FAVORITE,
        tr!("music.nav.songs"),
        Place::Liked,
    ));
    lines
}

fn playlists(music: &Music, lines: &mut Vec<Line>) {
    lines.push(Line::Heading(Heading::Playlists));
    match music.library().playlists() {
        Load::Loading => lines.push(Line::Loading),
        Load::Failed(why) => lines.push(Line::Failed(why.clone())),
        Load::Ready(listed) => {
            let service = music
                .connection()
                .map_or("", |connection| connection.service.slug);
            lines.extend(
                listed
                    .iter()
                    .filter(|playlist| !music.blends().mirrors(service, &playlist.id))
                    .cloned()
                    .map(Line::Playlist),
            );
        }
    }
}

fn heading_line(heading: Heading, syncing: bool, linked: bool) -> Element {
    let (title, refresh) = match heading {
        Heading::Blends => (
            tr!("music.nav.blends"),
            button(icon(MaterialSymbol::SYNC).extent(Length::px(16.0)))
                .ghost()
                .icon_only()
                .small()
                .enabled(linked && !syncing)
                .on_press(|music: &mut Music| music.sync_blends()),
        ),
        Heading::Playlists => (
            tr!("music.nav.playlists"),
            button(icon(MaterialSymbol::REFRESH).extent(Length::px(16.0)))
                .ghost()
                .icon_only()
                .small()
                .on_press(|library: &mut Library| library.refresh()),
        ),
    };
    let add = button(icon(MaterialSymbol::ADD).extent(Length::px(18.0)))
        .ghost()
        .icon_only()
        .small();
    let add = match heading {
        Heading::Blends => add.on_press(|music: &mut Music| music.new_blend(Arc::default())),
        Heading::Playlists => add.on_press(|music: &mut Music| music.new_playlist(Arc::default())),
    };
    row! {
        text(title).caption().muted().semibold().grow(1.0);
        add;
        refresh;
    }
    .align_items(Align::Center)
    .padding(Inset::edges(
        Space::SNUG,
        Space::SNUG,
        Space::TIGHT,
        Space::SNUG,
    ))
    .height(HEADING)
    .fill_width()
    .any()
}

fn blend_entry(blend: &Blend, chosen: bool) -> Element {
    let id = blend.id().clone();
    let artwork = blend.artwork();
    let body = entry(
        MediaLine {
            artwork: &artwork,
            side: 38.0,
            title: blend.name().to_owned(),
            subtitle: songs(blend.len()).to_string(),
            lit: chosen,
            trailing: None,
        },
        chosen,
    )
    .on_press(move |music: &mut Music| music.go(Place::Blend(id.clone())))
    .fill();
    menus::target(Subject::Blend(blend.id().clone()), body)
        .height(ENTRY)
        .fill_width()
}

fn playlist_entry(playlist: &Playlist, chosen: bool) -> Element {
    let id = playlist.id.clone();
    let body = entry(
        MediaLine {
            artwork: &playlist.artwork,
            side: 38.0,
            title: playlist.name.clone(),
            subtitle: playlist.owner.clone(),
            lit: chosen,
            trailing: None,
        },
        chosen,
    )
    .on_press(move |music: &mut Music| music.go(Place::Playlist(id.clone())))
    .fill();
    menus::target(Subject::Playlist(playlist.clone()), body)
        .height(ENTRY)
        .fill_width()
}

fn entry(line: MediaLine<'_>, chosen: bool) -> Container {
    container(line.view())
        .padding(Inset::xy(Space::SNUG, Space::TIGHT))
        .radius(Corner::Item)
        .background(match chosen {
            true => palette().accent.with_alpha(0.12),
            false => Color::TRANSPARENT,
        })
}

fn destination(symbol: MaterialSymbol, title: Label, place: Place, current: &Place) -> impl View {
    let chosen = *current == place;
    let tint = match chosen {
        true => palette().accent,
        false => palette().text,
    };
    let glyph = icon(symbol).extent(Length::px(20.0)).color(tint);
    container(
        row! {
            match chosen {
                true => glyph.filled(),
                false => glyph,
            };
            text(title).label().medium().color(tint).no_wrap();
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center),
    )
    .padding(Inset::xy(Space::NORMAL, Space::SNUG))
    .radius(Corner::Panel)
    .background(match chosen {
        true => palette().accent.with_alpha(0.14),
        false => Color::TRANSPARENT,
    })
    .on_press(move |music: &mut Music| music.root(place.clone()))
    .height(DESTINATION)
    .fill_width()
}

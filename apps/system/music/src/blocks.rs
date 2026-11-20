use std::collections::HashSet;
use std::sync::Arc;

use music_blend::{BlendId, EntryKey};
use music_core::{Album, Artist, Artwork, Card, Fidelity, Id, Playlist, Track};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::artwork::cover;
use crate::library::Library;
use crate::marks::mark;
use crate::menus::{self, Subject, TrackContext, TrackTarget};
use crate::music::Music;
use crate::nav::Place;
use crate::player::Player;
use crate::widgets::{bars, fidelity_label, high, lift};
use crate::wording::{self, length};

pub(crate) const ROW: f32 = 58.0;
pub(crate) const TILE: f32 = 60.0;
pub(crate) const CARD: f32 = 168.0;
pub(crate) const SHELF: f32 = 238.0;
pub(crate) const UNAVAILABLE: f32 = 0.45;
const THUMB: f32 = 42.0;
const INDEX: f32 = 36.0;
const TIME: f32 = 52.0;
const QUICK_WIDTH: f32 = 320.0;
const QUICK_ROWS: usize = 4;
const QUICK_HEIGHT: f32 = TILE * QUICK_ROWS as f32 + 12.0;
const QUICK_COLUMNS: usize = 6;
const SHELF_CARDS: usize = 16;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Listening {
    pub(crate) now: Option<(&'static str, Id)>,
    pub(crate) playing: bool,
    pub(crate) saved: Arc<HashSet<Id>>,
    pub(crate) service: Option<&'static str>,
}

impl Listening {
    pub(crate) fn new(music: &Music) -> Listening {
        Listening {
            now: music
                .player()
                .now()
                .map(|track| (track.service.slug, track.id.clone())),
            playing: music.player().playing(),
            saved: music.library().saved(),
            service: music.connection().map(|connection| connection.service.slug),
        }
    }

    pub(crate) fn is_now(&self, track: &Track) -> bool {
        self.now
            .as_ref()
            .is_some_and(|(service, id)| *service == track.service.slug && *id == track.id)
    }

    fn browses(&self, track: &Track) -> bool {
        self.service == Some(track.service.slug)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Within {
    Playlist(Id),
    Blend { id: BlendId, keys: Arc<[EntryKey]> },
}

impl Within {
    fn context(&self, index: usize) -> TrackContext {
        match self {
            Within::Playlist(id) => TrackContext::Playlist(id.clone()),
            Within::Blend { id, keys } => keys.get(index).map_or(TrackContext::Listed, |key| {
                TrackContext::Blend(id.clone(), *key)
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Leading {
    Number,
    Cover,
}

pub(crate) fn track_row(
    tracks: &Arc<[Track]>,
    index: usize,
    leading: Leading,
    within: Option<&Within>,
    listening: &Listening,
) -> Element {
    let Some(track) = tracks.get(index) else {
        return Element::empty();
    };
    let now = listening.is_now(track);
    let browsed = listening.browses(track);
    let saved = browsed && listening.saved.contains(&track.id);
    let marked = matches!(within, Some(Within::Blend { .. }));
    let title_color = match (now, track.playable) {
        (true, _) => palette().accent,
        (false, true) => palette().text,
        (false, false) => palette().text_muted,
    };

    let lead = match (now, leading) {
        (true, _) => container(bars(listening.playing, 16.0))
            .center()
            .width(INDEX)
            .any(),
        (false, Leading::Number) => text(match track.number {
            0 => (index + 1).to_string(),
            number => number.to_string(),
        })
        .caption()
        .muted()
        .align(TextAlign::Center)
        .width(INDEX)
        .any(),
        (false, Leading::Cover) => container(cover(&track.artwork, THUMB, 6.0))
            .width(INDEX + 12.0)
            .any(),
    };

    let list = Arc::clone(tracks);
    let heart_track = track.clone();
    let subject = Subject::Track(TrackTarget {
        track: track.clone(),
        context: within.map_or(TrackContext::Listed, |within| within.context(index)),
    });
    let more_key = ("music-track-more", track.uri(), index);
    let album = track.album.id.clone().filter(|_| browsed).map(|id| {
        let name = track.album.name.clone();
        container(text(name).caption().muted().no_wrap())
            .padding(Inset::xy(Space::TIGHT, Space::HAIR))
            .radius(Corner::Item)
            .on_press(move |music: &mut Music| music.go(Place::Album(id.clone())))
    });

    let body = container(
        row! {
            lead;
            column! {
                text(track.name.clone()).label().medium().no_wrap().color(title_color);
                row! {
                    if marked {
                        icon(mark(track.service.mark)).extent(Length::px(13.0)).color(palette().text_muted);
                    }
                    if track.explicit {
                        icon(MaterialSymbol::EXPLICIT).extent(Length::px(14.0)).color(palette().text_muted);
                    }
                    high(track.fidelity);
                    text(track.byline()).caption().muted().no_wrap();
                }
                .gap(Space::TIGHT)
                .align_items(Align::Center);
            }
            .gap(Space::HAIR)
            .grow(3.0)
            .shrink(1.0)
            .min_width(0.0);
            row! { album; }.grow(2.0).shrink(1.0).min_width(0.0);
            menus::more(subject.clone(), more_key);
            match browsed {
                true => heart(heart_track, saved).any(),
                false => spacer().width(ControlSize::Small.height()).any(),
            };
            text(length(track.duration))
                .caption()
                .muted()
                .align(TextAlign::End)
                .width(TIME);
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::xy(Space::SNUG, Space::TIGHT))
    .radius(Corner::Item)
    .background(match now {
        true => palette().accent.with_alpha(0.08),
        false => Color::TRANSPARENT,
    })
    .fill();
    let body = match track.playable {
        true => body
            .on_press(move |player: &mut Player| player.play(&list, index))
            .any(),
        false => body.opacity(UNAVAILABLE).any(),
    };
    menus::target(subject, body).height(ROW).fill_width()
}

pub(crate) fn heart(track: Track, saved: bool) -> Button {
    let symbol = icon(MaterialSymbol::FAVORITE)
        .extent(Length::px(18.0))
        .color(match saved {
            true => palette().accent,
            false => palette().text_muted,
        });
    let symbol = match saved {
        true => symbol.filled(),
        false => symbol,
    };
    button(symbol)
        .ghost()
        .icon_only()
        .small()
        .on_press(move |library: &mut Library| library.set_saved(&track, !saved))
}

pub(crate) fn columns() -> impl View {
    row! {
        text("#").caption().muted().align(TextAlign::Center).width(INDEX);
        text(tr!("music.column.title")).caption().muted().grow(3.0).min_width(0.0);
        text(tr!("music.column.album")).caption().muted().grow(2.0).min_width(0.0);
        spacer().width(72.0);
        icon(MaterialSymbol::SCHEDULE)
            .extent(Length::px(15.0))
            .color(palette().text_muted)
            .width(TIME);
    }
    .gap(Space::NORMAL)
    .padding(Inset::xy(Space::SNUG, Space::TIGHT))
    .align_items(Align::Center)
    .fill_width()
}

pub(crate) fn quick_picks(tracks: &Arc<[Track]>, listening: &Listening) -> Element {
    let columns = tracks.chunks(QUICK_ROWS).take(QUICK_COLUMNS).enumerate();
    fitted(
        columns.map(|(column, chunk)| {
            let start = column * QUICK_ROWS;
            column_of(
                (start..start + chunk.len()).map(|index| quick_tile(tracks, index, listening)),
            )
            .gap(Space::TIGHT)
            .width(QUICK_WIDTH)
            .height(Length::px(QUICK_HEIGHT))
        }),
        Space::ROOMY,
        QUICK_HEIGHT,
    )
}

fn fitted(cells: impl IntoIterator<Item = impl View>, gap: Space, height: f32) -> Element {
    row_of(cells)
        .wrap_items()
        .gap(gap)
        .height(Length::px(height))
        .fill_width()
        .clipped()
}

fn quick_tile(tracks: &Arc<[Track]>, index: usize, listening: &Listening) -> Element {
    let track = &tracks[index];
    let now = listening.is_now(track);
    let list = Arc::clone(tracks);
    let body = container(
        MediaLine {
            artwork: &track.artwork,
            side: 48.0,
            title: track.name.clone(),
            subtitle: track.byline(),
            lit: now,
            trailing: now.then(|| bars(listening.playing, 14.0).any()),
        }
        .view(),
    )
    .padding(Inset::all(Space::TIGHT))
    .radius(Corner::Item)
    .on_press(move |player: &mut Player| player.play(&list, index))
    .fill();
    menus::target(listed(track), body).height(TILE).fill_width()
}

fn listed(track: &Track) -> Subject {
    Subject::Track(TrackTarget {
        track: track.clone(),
        context: TrackContext::Listed,
    })
}

pub(crate) struct MediaLine<'a> {
    pub(crate) artwork: &'a Artwork,
    pub(crate) side: f32,
    pub(crate) title: String,
    pub(crate) subtitle: String,
    pub(crate) lit: bool,
    pub(crate) trailing: Option<Element>,
}

impl MediaLine<'_> {
    pub(crate) fn view(self) -> impl View {
        row! {
            cover(self.artwork, self.side, (self.side / 6.0).round());
            column! {
                text(self.title)
                    .label()
                    .medium()
                    .no_wrap()
                    .color(if self.lit { palette().accent } else { palette().text });
                text(self.subtitle).caption().muted().no_wrap();
            }
            .gap(Space::HAIR)
            .grow(1.0)
            .min_width(0.0);
            self.trailing;
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width()
    }
}

pub(crate) fn shelf<T>(items: &[T], card: fn(&T) -> Element) -> Element {
    fitted(
        items
            .iter()
            .take(SHELF_CARDS)
            .map(|item| container(card(item)).center().height(Length::px(SHELF))),
        Space::SNUG,
        SHELF,
    )
}

pub(crate) fn card(card: &Card) -> Element {
    match card {
        Card::Album(album) => album_card(album),
        Card::Artist(artist) => artist_card(artist),
        Card::Playlist(playlist) => playlist_card(playlist),
        Card::Mix(track) => mix_card(track),
        Card::Song(track) => song_card(track),
    }
}

pub(crate) fn album_card(album: &Album) -> Element {
    let id = album.id.clone();
    tile(
        &album.artwork,
        12.0,
        album.name.clone(),
        wording::release(album) + " · " + &album.byline(),
        high(album.fidelity),
        move |music: &mut Music| music.go(Place::Album(id.clone())),
    )
}

pub(crate) fn artist_card(artist: &Artist) -> Element {
    let id = artist.id.clone();
    tile(
        &artist.artwork,
        CARD / 2.0,
        artist.name.clone(),
        tr!("music.kind.artist").to_string(),
        None,
        move |music: &mut Music| music.go(Place::Artist(id.clone())),
    )
}

pub(crate) fn playlist_card(playlist: &Playlist) -> Element {
    let id = playlist.id.clone();
    let under = match playlist.owner.is_empty() {
        true => tr!("music.kind.playlist").to_string(),
        false => playlist.owner.clone(),
    };
    let card = tile(
        &playlist.artwork,
        12.0,
        playlist.name.clone(),
        under,
        None,
        move |music: &mut Music| music.go(Place::Playlist(id.clone())),
    );
    menus::target(Subject::Playlist(playlist.clone()), card)
}

pub(crate) fn mix_card(seed: &Track) -> Element {
    let place = Place::Mix(seed.clone());
    tile(
        &seed.artwork,
        12.0,
        tr!("music.mix.title", name = seed.byline().as_str()).to_string(),
        tr!("music.mix.seeded", name = seed.name.as_str()).to_string(),
        None,
        move |music: &mut Music| music.go(place.clone()),
    )
}

pub(crate) fn song_card(track: &Track) -> Element {
    let played = track.clone();
    let card = tile(
        &track.artwork,
        12.0,
        track.name.clone(),
        track.byline(),
        high(track.fidelity),
        move |music: &mut Music| music.listen(played.clone()),
    );
    menus::target(listed(track), card)
}

pub(crate) fn tile(
    artwork: &Artwork,
    radius: f32,
    title: String,
    under: String,
    badge: Option<Container>,
    open: impl Fn(&mut Music) + 'static,
) -> Element {
    lift(
        container(
            column! {
                cover(artwork, CARD - 16.0, radius);
                row! {
                    text(title).label().semibold().no_wrap().grow(1.0).min_width(0.0);
                    badge;
                }
                .gap(Space::TIGHT)
                .align_items(Align::Center)
                .fill_width();
                text(under).caption().muted().no_wrap().fill_width();
            }
            .gap(Space::TIGHT)
            .fill_width(),
        )
        .padding(Inset::all(Space::SNUG))
        .radius(Corner::Panel)
        .on_press(open)
        .width(Length::px(CARD)),
    )
}

pub(crate) fn heading(title: impl Into<Label>, above: Space) -> Element {
    container(text(title).heading().bold())
        .padding(Inset::edges(above, Space::TIGHT, Space::SNUG, Space::TIGHT))
        .fill_width()
        .any()
}

#[derive(Clone)]
pub(crate) struct Header {
    pub(crate) artwork: Artwork,
    pub(crate) round: bool,
    pub(crate) kind: Label,
    pub(crate) title: String,
    pub(crate) lines: Vec<String>,
    pub(crate) fidelity: Option<Fidelity>,
    pub(crate) tracks: Arc<[Track]>,
    pub(crate) menu: Option<Subject>,
}

const HERO: f32 = 196.0;

pub(crate) fn header(header: Header) -> Element {
    let radius = match header.round {
        true => HERO / 2.0,
        false => 16.0,
    };
    let playable = header.tracks.iter().any(|track| track.playable);
    let play_list = Arc::clone(&header.tracks);
    let shuffle_list = Arc::clone(&header.tracks);
    let more = header
        .menu
        .map(|subject| menus::more(subject, "music-header-more"));
    let quality = header.fidelity.map(fidelity_label);
    row! {
        cover(&header.artwork, HERO, radius).shrink(0.0);
        column! {
            text(header.kind).caption().muted().semibold();
            text(header.title).display().bold();
            column_of(header.lines.into_iter().map(|line| text(line).label().muted().build()))
                .gap(Space::HAIR);
            quality;
            spacer().height(Space::SNUG.px());
            row! {
                button(
                    row! {
                        icon(MaterialSymbol::PLAY_ARROW).filled().extent(Length::px(20.0));
                        text(tr!("music.play")).label().semibold();
                    }
                    .gap(Space::TIGHT)
                    .align_items(Align::Center),
                )
                .large()
                .enabled(playable)
                .on_press(move |player: &mut Player| player.play(&play_list, 0));
                button(
                    row! {
                        icon(MaterialSymbol::SHUFFLE).extent(Length::px(18.0));
                        text(tr!("music.shuffle")).label();
                    }
                    .gap(Space::TIGHT)
                    .align_items(Align::Center),
                )
                .secondary()
                .large()
                .enabled(playable)
                .on_press(move |player: &mut Player| player.shuffled(&shuffle_list));
                more;
            }
            .gap(Space::SNUG)
            .align_items(Align::Center);
        }
        .gap(Space::TIGHT)
        .justify(Justify::End)
        .grow(1.0)
        .min_width(0.0);
    }
    .gap(Space::LOOSE)
    .align_items(Align::End)
    .padding(Inset::edges(
        Space::WIDE,
        Space::SNUG,
        Space::ROOMY,
        Space::SNUG,
    ))
    .fill_width()
    .any()
}

pub(crate) fn waiting() -> Element {
    container(spinner()).center().fill().any()
}

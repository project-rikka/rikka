use std::sync::Arc;

use music_core::{Album, Artist, Card, Playlist, Track};
use rikka_app::prelude::*;
use rikka_widgets::row;

use crate::blocks::{Listening, album_card, artist_card, card, playlist_card, quick_picks, shelf};
use crate::music::Music;
use crate::nav::Place;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Gallery {
    pub(crate) title: Label,
    pub(crate) items: Items,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Items {
    Tracks(Arc<[Track]>),
    Cards(Cards),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Cards {
    Mixed(Arc<[Card]>),
    Albums(Arc<[Album]>),
    Artists(Arc<[Artist]>),
    Playlists(Arc<[Playlist]>),
}

impl Gallery {
    pub(crate) fn tracks(title: impl Into<Label>, tracks: &Arc<[Track]>) -> Gallery {
        Gallery {
            title: title.into(),
            items: Items::Tracks(Arc::clone(tracks)),
        }
    }

    pub(crate) fn cards(title: impl Into<Label>, cards: Cards) -> Gallery {
        Gallery {
            title: title.into(),
            items: Items::Cards(cards),
        }
    }

    pub(crate) fn heading(&self, above: Space) -> Element {
        let place = Place::Gallery(self.clone());
        row! {
            text(self.title.clone()).heading().bold().no_wrap().grow(1.0).min_width(0.0);
            button(text(tr!("music.gallery.all")).label().semibold())
                .ghost()
                .small()
                .on_press(move |music: &mut Music| music.go(place.clone()));
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .padding(Inset::edges(above, Space::TIGHT, Space::SNUG, Space::TIGHT))
        .fill_width()
        .any()
    }

    pub(crate) fn preview(&self, listening: &Listening) -> Element {
        match &self.items {
            Items::Tracks(tracks) => quick_picks(tracks, listening),
            Items::Cards(cards) => cards.preview(),
        }
    }
}

impl Cards {
    pub(crate) fn len(&self) -> usize {
        match self {
            Cards::Mixed(cards) => cards.len(),
            Cards::Albums(albums) => albums.len(),
            Cards::Artists(artists) => artists.len(),
            Cards::Playlists(playlists) => playlists.len(),
        }
    }

    pub(crate) fn card(&self, index: usize) -> Element {
        match self {
            Cards::Mixed(cards) => card(&cards[index]),
            Cards::Albums(albums) => album_card(&albums[index]),
            Cards::Artists(artists) => artist_card(&artists[index]),
            Cards::Playlists(playlists) => playlist_card(&playlists[index]),
        }
    }

    fn preview(&self) -> Element {
        match self {
            Cards::Mixed(cards) => shelf(cards, card),
            Cards::Albums(albums) => shelf(albums, album_card),
            Cards::Artists(artists) => shelf(artists, artist_card),
            Cards::Playlists(playlists) => shelf(playlists, playlist_card),
        }
    }
}

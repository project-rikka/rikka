use music_library::{AlbumSort, Order, SongSort};
use rikka_app::prelude::*;

#[derive(Debug, Default, State)]
pub(crate) struct Arrange {
    albums: Tracked<AlbumSort>,
    songs: Tracked<(SongSort, Order)>,
    artist_grid: Tracked<bool>,
}

impl Arrange {
    pub(crate) fn albums(&self) -> AlbumSort {
        *self.albums
    }

    pub(crate) fn songs(&self) -> (SongSort, Order) {
        *self.songs
    }

    pub(crate) fn artist_grid(&self) -> bool {
        *self.artist_grid
    }

    pub(crate) fn sort_albums(&mut self, sort: AlbumSort) {
        if *self.albums != sort {
            self.albums.set(sort);
        }
    }

    pub(crate) fn sort_songs(&mut self, sort: SongSort) {
        let (current, order) = *self.songs;
        let order = match (current == sort, order) {
            (true, Order::Ascending) => Order::Descending,
            (true, Order::Descending) => Order::Ascending,
            (false, _) => first_order(sort),
        };
        self.songs.set((sort, order));
    }

    pub(crate) fn show_artist_grid(&mut self, grid: bool) {
        if *self.artist_grid != grid {
            self.artist_grid.set(grid);
        }
    }
}

pub(crate) fn album_order(sort: AlbumSort) -> Order {
    match sort {
        AlbumSort::Artist | AlbumSort::Title => Order::Ascending,
        AlbumSort::Year | AlbumSort::Added => Order::Descending,
    }
}

fn first_order(sort: SongSort) -> Order {
    match sort {
        SongSort::Title
        | SongSort::Artist
        | SongSort::Album
        | SongSort::Duration
        | SongSort::Codec => Order::Ascending,
        SongSort::Year | SongSort::Added | SongSort::Plays => Order::Descending,
    }
}

use crate::ids::{ArtistId, GenreId};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AlbumSort {
    #[default]
    Artist,
    Title,
    Year,
    Added,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SongSort {
    #[default]
    Title,
    Artist,
    Album,
    Year,
    Duration,
    Added,
    Plays,
    Codec,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Order {
    #[default]
    Ascending,
    Descending,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[must_use]
pub struct Albums {
    pub(crate) sort: AlbumSort,
    pub(crate) order: Order,
    pub(crate) genre: Option<GenreId>,
    pub(crate) artist: Option<ArtistId>,
}

impl Albums {
    pub fn new(sort: AlbumSort) -> Albums {
        Albums {
            sort,
            ..Albums::default()
        }
    }

    pub fn order(mut self, order: Order) -> Albums {
        self.order = order;
        self
    }

    pub fn genre(mut self, genre: GenreId) -> Albums {
        self.genre = Some(genre);
        self
    }

    pub fn artist(mut self, artist: ArtistId) -> Albums {
        self.artist = Some(artist);
        self
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[must_use]
pub struct Songs {
    pub(crate) sort: SongSort,
    pub(crate) order: Order,
}

impl Songs {
    pub fn new(sort: SongSort) -> Songs {
        Songs {
            sort,
            order: Order::Ascending,
        }
    }

    pub fn order(mut self, order: Order) -> Songs {
        self.order = order;
        self
    }
}

impl Order {
    pub(crate) fn sql(self) -> &'static str {
        match self {
            Order::Ascending => "ASC",
            Order::Descending => "DESC",
        }
    }
}

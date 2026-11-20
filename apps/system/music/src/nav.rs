use std::path::PathBuf;

use music_blend::BlendId;
use music_core::{Id, Track};
use music_library::GenreId;
use rikka_app::prelude::*;

use crate::gallery::Gallery;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Place {
    Home,
    Search,
    Liked,
    Albums,
    Artists,
    Songs,
    Genres,
    Folders,
    Genre { id: GenreId, name: String },
    Folder { root: PathBuf, path: PathBuf },
    Playlist(Id),
    Blend(BlendId),
    Album(Id),
    Artist(Id),
    Mix(Track),
    Gallery(Gallery),
}

impl Place {
    pub(crate) fn root(&self) -> usize {
        match self {
            Place::Home => 0,
            Place::Search => 1,
            Place::Albums => 2,
            Place::Artists => 3,
            Place::Songs => 4,
            Place::Genres => 5,
            Place::Folders => 6,
            Place::Liked => 7,
            Place::Genre { .. }
            | Place::Folder { .. }
            | Place::Playlist(_)
            | Place::Blend(_)
            | Place::Album(_)
            | Place::Artist(_)
            | Place::Mix(_)
            | Place::Gallery(_) => 8,
        }
    }
}

#[derive(State)]
pub(crate) struct Nav {
    trail: Tracked<Vec<Place>>,
    ahead: Tracked<Vec<Place>>,
}

impl Nav {
    pub(crate) fn new() -> Nav {
        Nav {
            trail: Tracked::new(vec![Place::Home]),
            ahead: Tracked::new(Vec::new()),
        }
    }

    pub(crate) fn current(&self) -> &Place {
        self.trail.last().unwrap_or(&Place::Home)
    }

    pub(crate) fn origin(&self) -> &Place {
        self.trail.first().unwrap_or(&Place::Home)
    }

    pub(crate) fn trail_position(&self) -> (usize, usize) {
        let root = self.trail.first().map_or(0, Place::root);
        (root, self.trail.len())
    }

    pub(crate) fn can_go_back(&self) -> bool {
        self.trail.len() > 1
    }

    pub(crate) fn can_go_forward(&self) -> bool {
        !self.ahead.is_empty()
    }

    pub(crate) fn go(&mut self, place: Place) {
        if self.current() == &place {
            return;
        }
        self.trail.push(place);
        self.ahead.set(Vec::new());
    }

    pub(crate) fn root(&mut self, place: Place) {
        if self.trail.len() == 1 && self.current() == &place {
            return;
        }
        self.trail.set(vec![place]);
        self.ahead.set(Vec::new());
    }

    pub(crate) fn back(&mut self) {
        if self.trail.len() > 1
            && let Some(left) = self.trail.pop()
        {
            self.ahead.push(left);
        }
    }

    pub(crate) fn forward(&mut self) {
        if let Some(place) = self.ahead.pop() {
            self.trail.push(place);
        }
    }

    pub(crate) fn holds(&self, place: &Place) -> bool {
        self.trail.contains(place) || self.ahead.contains(place)
    }

    pub(crate) fn forget(&mut self, place: &Place) {
        if !self.holds(place) {
            return;
        }
        let mut trail: Vec<Place> = self
            .trail
            .iter()
            .filter(|visited| *visited != place)
            .cloned()
            .collect();
        trail.dedup();
        if trail.is_empty() {
            trail.push(Place::Home);
        }
        let ahead = self
            .ahead
            .iter()
            .filter(|visited| *visited != place)
            .cloned()
            .collect();
        self.trail.set(trail);
        self.ahead.set(ahead);
    }
}

impl std::fmt::Debug for Nav {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Nav")
            .field("current", self.current())
            .finish_non_exhaustive()
    }
}

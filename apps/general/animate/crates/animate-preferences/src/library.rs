use std::path::PathBuf;

use animate_library::{Layout, Location, Shelf, ThumbnailSize};
use rikka_settings::{Key, Role, Schema, Value};
use serde::{Deserialize, Serialize};

use super::Preferences;

const LOCATIONS: &str = "library-locations";
const SHELF: &str = "library-shelf";
const THUMBNAIL_SIZE: &str = "library-thumbnail-size";

#[derive(Default, Serialize, Deserialize)]
struct StoredLocation {
    path: String,
    layout: String,
}

impl Preferences {
    pub fn library_locations(&self) -> Vec<Location> {
        let stored = self.0.borrow().get::<Vec<StoredLocation>>(LOCATIONS);
        stored
            .into_iter()
            .filter_map(|location| {
                let layout = Layout::from_slug(&location.layout)?;
                let root = PathBuf::from(location.path);
                root.is_absolute().then_some(Location { root, layout })
            })
            .collect()
    }

    pub fn keep_library_locations(&self, locations: &[Location]) {
        let stored: Vec<StoredLocation> = locations
            .iter()
            .filter_map(|location| {
                let path = location.root.to_str();
                if path.is_none() {
                    tracing::warn!(path = %location.root.display(), "library folder name is not unicode, not kept");
                }
                Some(StoredLocation {
                    path: path?.to_owned(),
                    layout: location.layout.slug().to_owned(),
                })
            })
            .collect();
        self.0.borrow_mut().set(LOCATIONS, stored);
    }

    pub fn library_shelf(&self) -> Shelf {
        self.0.borrow().choice(SHELF)
    }

    pub fn keep_library_shelf(&self, shelf: Shelf) {
        self.0.borrow_mut().set_choice(SHELF, shelf);
    }

    pub fn thumbnail_size(&self) -> ThumbnailSize {
        self.0.borrow().choice(THUMBNAIL_SIZE)
    }

    pub fn keep_thumbnail_size(&self, size: ThumbnailSize) {
        self.0.borrow_mut().set_choice(THUMBNAIL_SIZE, size);
    }
}

pub(super) fn keys(schema: Schema) -> Schema {
    schema
        .key(
            Key::list(LOCATIONS, std::iter::empty::<Value>())
                .summary("Folders added to the library, each a whole library or one shelf"),
        )
        .key(
            Key::choice::<Shelf>(SHELF)
                .summary("The library shelf shown last")
                .role(Role::State),
        )
        .key(Key::choice::<ThumbnailSize>(THUMBNAIL_SIZE).summary("Size of library thumbnails"))
}

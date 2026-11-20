use rikka_sync::RwLock;
use std::collections::HashMap;
use std::sync::LazyLock;

use rikka_text_font::Location;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LocationId(u32);

#[derive(Default)]
struct Interned {
    locations: Vec<&'static Location>,
    ids: HashMap<&'static Location, LocationId>,
}

static INTERNED: LazyLock<RwLock<Interned>> = LazyLock::new(|| {
    let default: &'static Location = Box::leak(Box::default());
    RwLock::new(Interned {
        locations: vec![default],
        ids: HashMap::from([(default, LocationId::DEFAULT)]),
    })
});

impl LocationId {
    pub const DEFAULT: Self = Self(0);

    pub fn location(self) -> &'static Location {
        INTERNED.read().locations[self.0 as usize]
    }

    pub fn is_default(self) -> bool {
        self == Self::DEFAULT
    }

    pub(crate) fn intern(location: Location) -> Self {
        if location.is_default() {
            return Self::DEFAULT;
        }
        if let Some(&id) = INTERNED.read().ids.get(&location) {
            return id;
        }
        let mut interned = INTERNED.write();
        if let Some(&id) = interned.ids.get(&location) {
            return id;
        }
        let id = Self(interned.locations.len() as u32);
        let leaked: &'static Location = Box::leak(Box::new(location));
        interned.locations.push(leaked);
        interned.ids.insert(leaked, id);
        id
    }
}

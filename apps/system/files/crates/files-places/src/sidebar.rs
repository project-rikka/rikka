use files_core::{FolderKind, Location, SCHEME_TRASH};
use files_drives::{Volume, VolumeKind};
use rikka_dirs::UserDirs;

use crate::{bookmarks, user_dirs};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    pub at: Option<Location>,
    pub name: Option<String>,
    pub icon: PlaceIcon,
    pub group: Group,
    pub volume: Option<Volume>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaceIcon {
    Home,
    Desktop,
    Documents,
    Downloads,
    Music,
    Pictures,
    Videos,
    Templates,
    Shared,
    Trash,
    Pin,
    Disk,
    Removable,
    Disc,
    Locked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    Personal,
    Pinned,
    Devices,
}

#[derive(Clone, Debug, Default)]
pub struct Sidebar {
    pub places: Vec<Place>,
    user_dirs: UserDirs,
}

impl Sidebar {
    #[must_use]
    pub fn load(volumes: &[Volume]) -> Self {
        let user_dirs = UserDirs::load();
        let mut places = Vec::new();

        if let Some(home) = user_dirs.home() {
            places.push(Place {
                name: None,
                at: Some(Location::from_path(home)),
                icon: PlaceIcon::Home,
                group: Group::Personal,
                volume: None,
            });
        }
        for which in user_dirs::SIDEBAR {
            let Some(path) = user_dirs.path(which).filter(|path| path.is_dir()) else {
                continue;
            };
            let at = Location::from_path(path);
            places.push(Place {
                name: Some(at.name()),
                at: Some(at),
                icon: user_dirs::icon(which),
                group: Group::Personal,
                volume: None,
            });
        }
        places.push(Place {
            at: Some(Location::root_of(SCHEME_TRASH)),
            name: None,
            icon: PlaceIcon::Trash,
            group: Group::Personal,
            volume: None,
        });

        for pin in bookmarks::load() {
            places.push(Place {
                name: Some(pin.name),
                at: Some(pin.at),
                icon: PlaceIcon::Pin,
                group: Group::Pinned,
                volume: None,
            });
        }

        for volume in volumes {
            places.push(Place {
                at: volume.mount.as_ref().map(Location::from_path),
                name: None,
                icon: icon_for(volume),
                group: Group::Devices,
                volume: Some(volume.clone()),
            });
        }

        Self { places, user_dirs }
    }

    #[must_use]
    pub fn folder_kind(&self, at: &Location) -> Option<FolderKind> {
        let named = self.user_dirs.at(&at.to_path()?)?;
        Some(user_dirs::folder_kind(named))
    }

    #[must_use]
    pub fn is_pinned(&self, at: &Location) -> bool {
        self.places
            .iter()
            .any(|place| place.group == Group::Pinned && place.at.as_ref() == Some(at))
    }
}

fn icon_for(volume: &Volume) -> PlaceIcon {
    if volume.locked {
        return PlaceIcon::Locked;
    }
    match volume.kind {
        VolumeKind::Optical => PlaceIcon::Disc,
        VolumeKind::Removable => PlaceIcon::Removable,
        VolumeKind::Internal => PlaceIcon::Disk,
    }
}

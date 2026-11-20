use std::collections::HashSet;

use files_core::Location;
use files_drives::{Actions, VolumeId};
use files_places::{Group, PlaceIcon, Sidebar};
use rikka_ui::MaterialSymbol;

use crate::devices::Command;
use crate::wording;

#[derive(Clone, Debug)]
pub(crate) enum Kind {
    Heading,
    Fixed,
    Pin,
    Device(Device),
}

#[derive(Clone, Debug)]
pub(crate) struct Device {
    pub(crate) id: VolumeId,
    pub(crate) actions: Actions,
}

#[derive(Clone, Debug)]
pub(crate) enum Intent {
    Go(Location),
    Mount(VolumeId),
    Unlock(VolumeId),
    Nothing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Detach {
    Eject,
    Lock,
    Unmount,
}

impl Detach {
    pub(crate) fn command(self, id: VolumeId) -> Command {
        match self {
            Self::Eject => Command::Eject(id),
            Self::Lock => Command::Lock(id),
            Self::Unmount => Command::Unmount(id),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Row {
    pub(crate) label: String,
    pub(crate) icon: MaterialSymbol,
    pub(crate) kind: Kind,
    pub(crate) at: Option<Location>,
    pub(crate) busy: bool,
}

impl Row {
    pub(crate) fn is_heading(&self) -> bool {
        matches!(self.kind, Kind::Heading)
    }

    pub(crate) fn device(&self) -> Option<&Device> {
        match &self.kind {
            Kind::Device(device) => Some(device),
            _ => None,
        }
    }

    pub(crate) fn is_pin(&self) -> bool {
        matches!(self.kind, Kind::Pin)
    }

    pub(crate) fn intent(&self) -> Intent {
        match (self.device(), self.at.clone()) {
            (Some(device), _) if device.actions.unlock => Intent::Unlock(device.id.clone()),
            (_, Some(at)) => Intent::Go(at),
            (Some(device), None) if device.actions.mount => Intent::Mount(device.id.clone()),
            _ => Intent::Nothing,
        }
    }

    pub(crate) fn detach(&self) -> Option<Detach> {
        let device = self.device()?;
        if device.actions.eject {
            return Some(Detach::Eject);
        }
        if device.actions.lock {
            return Some(Detach::Lock);
        }
        device.actions.unmount.then_some(Detach::Unmount)
    }

    pub(crate) fn trailing(&self) -> Option<MaterialSymbol> {
        Some(match self.detach()? {
            Detach::Lock => MaterialSymbol::LOCK,
            Detach::Eject | Detach::Unmount => MaterialSymbol::EJECT,
        })
    }
}

pub(crate) fn rows(sidebar: &Sidebar, busy: &HashSet<VolumeId>) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut last: Option<Group> = None;

    for place in &sidebar.places {
        if last != Some(place.group)
            && let Some(heading) = wording::group(place.group)
        {
            rows.push(Row {
                label: heading,
                icon: MaterialSymbol::FOLDER,
                kind: Kind::Heading,
                at: None,
                busy: false,
            });
        }
        last = Some(place.group);

        let kind = match (&place.volume, place.group) {
            (Some(volume), _) => Kind::Device(Device {
                id: volume.id.clone(),
                actions: volume.actions,
            }),
            (None, Group::Pinned) => Kind::Pin,
            (None, _) => Kind::Fixed,
        };
        rows.push(Row {
            label: wording::place(place),
            icon: symbol_of(place.icon),
            busy: place
                .volume
                .as_ref()
                .is_some_and(|volume| busy.contains(&volume.id)),
            kind,
            at: place.at.clone(),
        });
    }
    rows
}

fn symbol_of(icon: PlaceIcon) -> MaterialSymbol {
    match icon {
        PlaceIcon::Home => MaterialSymbol::HOME,
        PlaceIcon::Desktop => MaterialSymbol::DESKTOP_WINDOWS,
        PlaceIcon::Documents => MaterialSymbol::DESCRIPTION,
        PlaceIcon::Downloads => MaterialSymbol::DOWNLOAD,
        PlaceIcon::Music => MaterialSymbol::LIBRARY_MUSIC,
        PlaceIcon::Pictures => MaterialSymbol::IMAGE,
        PlaceIcon::Videos => MaterialSymbol::MOVIE,
        PlaceIcon::Templates => MaterialSymbol::STICKY_NOTE_2,
        PlaceIcon::Shared => MaterialSymbol::FOLDER_SHARED,
        PlaceIcon::Trash => MaterialSymbol::DELETE,
        PlaceIcon::Pin => MaterialSymbol::PUSH_PIN,
        PlaceIcon::Disk => MaterialSymbol::HARD_DRIVE,
        PlaceIcon::Removable => MaterialSymbol::USB,
        PlaceIcon::Disc => MaterialSymbol::ALBUM,
        PlaceIcon::Locked => MaterialSymbol::LOCK,
    }
}

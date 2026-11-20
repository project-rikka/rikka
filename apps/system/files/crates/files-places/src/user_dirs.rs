use files_core::FolderKind;
use rikka_dirs::UserDir;

use crate::sidebar::PlaceIcon;

pub(crate) const SIDEBAR: [UserDir; 6] = [
    UserDir::Desktop,
    UserDir::Downloads,
    UserDir::Documents,
    UserDir::Pictures,
    UserDir::Music,
    UserDir::Videos,
];

#[must_use]
pub(crate) const fn folder_kind(which: UserDir) -> FolderKind {
    match which {
        UserDir::Pictures => FolderKind::Pictures,
        UserDir::Music => FolderKind::Music,
        UserDir::Videos => FolderKind::Videos,
        UserDir::Documents => FolderKind::Documents,
        UserDir::Desktop | UserDir::Downloads | UserDir::Templates | UserDir::PublicShare => {
            FolderKind::General
        }
    }
}

#[must_use]
pub(crate) const fn icon(which: UserDir) -> PlaceIcon {
    match which {
        UserDir::Desktop => PlaceIcon::Desktop,
        UserDir::Documents => PlaceIcon::Documents,
        UserDir::Downloads => PlaceIcon::Downloads,
        UserDir::Music => PlaceIcon::Music,
        UserDir::Pictures => PlaceIcon::Pictures,
        UserDir::Videos => PlaceIcon::Videos,
        UserDir::Templates => PlaceIcon::Templates,
        UserDir::PublicShare => PlaceIcon::Shared,
    }
}

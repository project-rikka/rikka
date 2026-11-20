use std::collections::HashMap;
use std::path::PathBuf;

use windows::Win32::UI::Shell::{
    FOLDERID_Desktop, FOLDERID_Documents, FOLDERID_Downloads, FOLDERID_Music, FOLDERID_Pictures,
    FOLDERID_Public, FOLDERID_Templates, FOLDERID_Videos,
};
use windows::core::GUID;

use super::UserDir;
use crate::known;

pub(super) fn named() -> HashMap<UserDir, PathBuf> {
    UserDir::ALL
        .into_iter()
        .filter_map(|which| Some((which, known::path(folder(which))?)))
        .collect()
}

fn folder(which: UserDir) -> &'static GUID {
    match which {
        UserDir::Desktop => &FOLDERID_Desktop,
        UserDir::Documents => &FOLDERID_Documents,
        UserDir::Downloads => &FOLDERID_Downloads,
        UserDir::Music => &FOLDERID_Music,
        UserDir::Pictures => &FOLDERID_Pictures,
        UserDir::Videos => &FOLDERID_Videos,
        UserDir::Templates => &FOLDERID_Templates,
        UserDir::PublicShare => &FOLDERID_Public,
    }
}

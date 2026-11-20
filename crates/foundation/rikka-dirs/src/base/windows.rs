use std::path::PathBuf;

use windows::Win32::UI::Shell::{
    FOLDERID_LocalAppData, FOLDERID_Profile, FOLDERID_ProgramData, FOLDERID_RoamingAppData,
};

use crate::known;

const CACHE: &str = "cache";

pub(super) fn home() -> Option<PathBuf> {
    known::path(&FOLDERID_Profile)
}

pub(super) fn data_home() -> Option<PathBuf> {
    known::path(&FOLDERID_LocalAppData)
}

pub(super) fn config_home() -> Option<PathBuf> {
    known::path(&FOLDERID_RoamingAppData)
}

pub(super) fn cache_home() -> Option<PathBuf> {
    known::path(&FOLDERID_LocalAppData)
}

pub(super) fn state_home() -> Option<PathBuf> {
    known::path(&FOLDERID_LocalAppData)
}

pub(super) fn runtime_dir() -> Option<PathBuf> {
    None
}

pub(super) fn system_data() -> Vec<PathBuf> {
    known::path(&FOLDERID_ProgramData).into_iter().collect()
}

pub(super) fn system_config() -> Vec<PathBuf> {
    known::path(&FOLDERID_ProgramData).into_iter().collect()
}

pub(super) fn app_cache(app: &str) -> Option<PathBuf> {
    Some(super::data_home()?.join(app).join(CACHE))
}

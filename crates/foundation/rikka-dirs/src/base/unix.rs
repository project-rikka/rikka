use std::path::PathBuf;

const SYSTEM_DATA: [&str; 3] = [
    "/run/current-system/sw/share",
    "/usr/local/share",
    "/usr/share",
];

const SYSTEM_CONFIG: &str = "/etc/xdg";

pub(super) fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|home| home.is_absolute())
}

pub(super) fn data_home() -> Option<PathBuf> {
    Some(super::home()?.join(".local/share"))
}

pub(super) fn config_home() -> Option<PathBuf> {
    Some(super::home()?.join(".config"))
}

pub(super) fn cache_home() -> Option<PathBuf> {
    Some(super::home()?.join(".cache"))
}

pub(super) fn state_home() -> Option<PathBuf> {
    Some(super::home()?.join(".local/state"))
}

pub(super) fn runtime_dir() -> Option<PathBuf> {
    super::named("XDG_RUNTIME_DIR")
}

pub(super) fn system_data() -> Vec<PathBuf> {
    SYSTEM_DATA.iter().map(PathBuf::from).collect()
}

pub(super) fn system_config() -> Vec<PathBuf> {
    vec![PathBuf::from(SYSTEM_CONFIG)]
}

pub(super) fn app_cache(app: &str) -> Option<PathBuf> {
    Some(cache_home()?.join(app))
}

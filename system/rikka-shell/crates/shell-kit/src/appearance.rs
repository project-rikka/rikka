use rikka_seam::Request;
use rikka_settings::Value;
use rikka_settings::seam::protocol;
use rikka_settings::session::appearance;
pub use rikka_settings::session::appearance::{
    DOMAIN, ICON_THEME, TASKBAR, TASKBAR_DEFAULT, TASKBAR_INDICATORS, TASKBAR_POSITION, THEME,
    TaskbarPosition, WALLPAPER,
};
use rikka_types::Choice;

use crate::Shell;

pub fn icon_theme(value: String) -> String {
    match value == appearance::UNSET {
        true => appearance::ICON_THEME_DEFAULT.to_owned(),
        false => value,
    }
}

pub fn redressed(event: &rikka_seam::Event) -> bool {
    moved_any(event, &[THEME]) || regenerated(event) || reglazed(event)
}

fn regenerated(event: &rikka_seam::Event) -> bool {
    moved_any(event, &appearance::MATERIAL_KEYS)
}

fn reglazed(event: &rikka_seam::Event) -> bool {
    moved_any(event, &appearance::GLASS_KEYS)
}

fn moved_any(event: &rikka_seam::Event, keys: &[&str]) -> bool {
    protocol::changed(event, DOMAIN)
        .is_some_and(|changed| keys.iter().any(|key| changed.changes_key(key)))
}

pub fn wallpaper(shell: &Shell) -> Option<String> {
    wallpaper_id(shell.setting::<String>(DOMAIN, WALLPAPER)?)
}

pub fn wallpaper_id(value: String) -> Option<String> {
    (value != appearance::UNSET).then_some(value)
}

pub fn taskbar(shell: &Shell) -> bool {
    shell
        .setting::<bool>(DOMAIN, TASKBAR)
        .unwrap_or(appearance::TASKBAR_DEFAULT)
}

pub fn taskbar_position(shell: &Shell) -> TaskbarPosition {
    shell
        .setting::<String>(DOMAIN, TASKBAR_POSITION)
        .map_or_else(TaskbarPosition::default, |slug| {
            TaskbarPosition::of_slug(&slug)
        })
}

pub fn write(key: &str, value: impl Into<Value>) -> Request {
    protocol::set(DOMAIN, key, value)
}

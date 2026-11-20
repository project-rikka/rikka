use rikka_sync::Mutex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::sfnt::without_table;

const STRIPPED_TABLE_SEPARATOR: char = '~';
const CONFIG_VARIABLE: &str = "FONTCONFIG_FILE";
const DIR_OPEN: &str = "<dir";
const DIR_CLOSE: &str = "</dir>";
const HOME_PREFIX: &str = "~/";

type LoadedFonts = Mutex<HashMap<String, Option<&'static [u8]>>>;

pub fn font_files() -> &'static [PathBuf] {
    static FILES: OnceLock<Vec<PathBuf>> = OnceLock::new();
    FILES.get_or_init(|| {
        let mut files = Vec::new();
        for dir in font_dirs() {
            walk(&dir, &mut files);
        }
        files
    })
}

pub fn find_font(file_name: &str) -> Option<&'static Path> {
    font_files()
        .iter()
        .find(|path| path.file_name().is_some_and(|name| name == file_name))
        .map(PathBuf::as_path)
}

pub fn font_path(file_name: &str) -> &'static Path {
    find_font(file_name).unwrap_or_else(|| panic!("font {file_name} is not installed"))
}

pub fn font_bytes(file_name: &str) -> Vec<u8> {
    let path = font_path(file_name);
    std::fs::read(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

pub fn loaded_font(file_name: &str) -> &'static [u8] {
    find_loaded_font(file_name).unwrap_or_else(|| panic!("font {file_name} is not installed"))
}

pub fn find_loaded_font(file_name: &str) -> Option<&'static [u8]> {
    static LOADED: OnceLock<LoadedFonts> = OnceLock::new();
    let mut loaded = LOADED.get_or_init(Mutex::default).lock();
    *loaded
        .entry(file_name.to_owned())
        .or_insert_with(|| load(file_name).map(|data| &*data.leak()))
}

fn load(file_name: &str) -> Option<Vec<u8>> {
    if let Some((base, table)) = file_name.split_once(STRIPPED_TABLE_SEPARATOR) {
        let tag: &[u8; 4] = table.as_bytes().try_into().ok()?;
        return without_table(&load(base)?, tag);
    }
    std::fs::read(find_font(file_name)?).ok()
}

fn font_dirs() -> Vec<PathBuf> {
    let config =
        std::env::var_os(CONFIG_VARIABLE).unwrap_or_else(|| panic!("{CONFIG_VARIABLE} is not set"));
    let text = std::fs::read_to_string(&config)
        .unwrap_or_else(|error| panic!("{}: {error}", Path::new(&config).display()));
    let home = rikka_dirs::home();
    let mut dirs = Vec::new();
    let mut rest = text.as_str();
    while let Some(start) = rest.find(DIR_OPEN) {
        rest = &rest[start..];
        let (Some(open_end), Some(close)) = (rest.find('>'), rest.find(DIR_CLOSE)) else {
            break;
        };
        let attributes = &rest[..open_end];
        let value = rest[open_end + 1..close].trim();
        rest = &rest[close..];
        if attributes.contains("prefix=") {
            continue;
        }
        match (value.strip_prefix(HOME_PREFIX), home) {
            (Some(relative), Some(home)) => dirs.push(home.join(relative)),
            (Some(_), None) => {}
            (None, _) => dirs.push(PathBuf::from(value)),
        }
    }
    dirs
}

fn walk(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, files);
        } else {
            files.push(path);
        }
    }
}

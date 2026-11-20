mod families;
mod folders;
mod registry;

use std::path::PathBuf;

use windows::Win32::Globalization::GetUserDefaultLocaleName;
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use windows::Win32::System::SystemServices::LOCALE_NAME_MAX_LENGTH;

use crate::config::{Environment, Fontconfig};
use crate::locale::Locale;
use crate::report::LoadProblem;
use crate::system::{EmbeddedFont, SystemFonts};

pub(crate) use families::{fallback_families, preferred_families};

pub(crate) fn fonts(_: &Environment, problems: &mut Vec<LoadProblem>) -> SystemFonts {
    let embedded = rikka_embedded_fonts::UI
        .iter()
        .map(|font| EmbeddedFont {
            name: font.name,
            bytes: font.bytes,
        })
        .collect();
    let mut files: Vec<PathBuf> = Vec::new();
    let mut dirs = Vec::new();
    match folders::system_fonts() {
        Ok(system) => {
            for root in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
                match registry::font_files(root, &system) {
                    Ok(listed) => files.extend(listed),
                    Err(source) => problems.push(LoadProblem::SystemFonts(source)),
                }
            }
            dirs.push(system);
        }
        Err(source) => problems.push(LoadProblem::SystemFonts(source)),
    }
    match folders::user_fonts() {
        Ok(user) => dirs.push(user),
        Err(source) => problems.push(LoadProblem::SystemFonts(source)),
    }
    SystemFonts {
        fontconfig: Fontconfig {
            dirs,
            aliases: families::aliases(),
            ..Fontconfig::default()
        },
        files,
        embedded,
    }
}

pub(crate) fn locale() -> Option<Locale> {
    let mut name = [0u16; LOCALE_NAME_MAX_LENGTH as usize];
    let length = unsafe { GetUserDefaultLocaleName(&mut name) };
    let length = usize::try_from(length).ok()?.checked_sub(1)?;
    Locale::parse(&String::from_utf16_lossy(&name[..length]))
}

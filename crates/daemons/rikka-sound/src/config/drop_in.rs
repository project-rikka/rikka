use std::path::PathBuf;

use crate::error::{Error, Result};

const FILE: &str = "99-rikka.conf";

pub(crate) const PREAMBLE: &str = "# Written by the Rikka settings. Anything else in here is lost the\n\
     # next time they are changed -- put your own settings in another file\n\
     # of this directory instead.\n";

#[derive(Debug)]
pub(crate) struct DropIn {
    path: PathBuf,
}

impl DropIn {
    pub(crate) fn in_directory(directory: &str) -> Option<Self> {
        Some(Self {
            path: rikka_dirs::config_home()?.join(directory).join(FILE),
        })
    }

    pub(crate) fn read(&self) -> String {
        std::fs::read_to_string(&self.path).unwrap_or_default()
    }

    pub(crate) fn write(&self, body: &str) -> Result<()> {
        if body.trim().is_empty() {
            return self.remove();
        }
        rikka_fs::replace(&self.path, format!("{PREAMBLE}\n{body}"))
            .map_err(|error| self.unwritable(&error.to_string()))
    }

    fn remove(&self) -> Result<()> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(self.unwritable(&error.to_string())),
        }
    }

    fn unwritable(&self, reason: &str) -> Error {
        Error::Unwritable {
            path: self.path.display().to_string(),
            reason: reason.to_owned(),
        }
    }
}

pub(crate) fn setting<'a>(body: &'a str, key: &str) -> Option<&'a str> {
    body.lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .find_map(|line| {
            let (name, value) = line.split_once('=')?;
            (name.trim() == key).then(|| value.trim())
        })
}

pub(crate) fn number(body: &str, key: &str) -> Option<u32> {
    rikka_pipewire::number(setting(body, key)?)
}

pub(crate) fn numbers(body: &str, key: &str) -> Vec<u32> {
    setting(body, key)
        .map(rikka_pipewire::numbers)
        .unwrap_or_default()
}

pub(crate) fn text<'a>(body: &'a str, key: &str) -> Option<&'a str> {
    Some(setting(body, key)?.trim_matches('"'))
}

pub(crate) fn format_list(numbers: &[u32]) -> String {
    let numbers: Vec<String> = numbers.iter().map(u32::to_string).collect();
    format!("[ {} ]", numbers.join(" "))
}

pub(crate) fn line(
    indent: &str,
    key: &str,
    value: Option<impl std::fmt::Display>,
) -> Option<String> {
    value.map(|value| format!("{indent}{key} = {value}"))
}

pub(crate) fn list(numbers: &[u32]) -> Option<String> {
    (!numbers.is_empty()).then(|| format_list(numbers))
}

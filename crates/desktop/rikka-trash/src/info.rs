use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Info {
    path: PathBuf,
    deleted: String,
}

impl Info {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self {
            path,
            deleted: stamp(SystemTime::now()),
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn to_file(&self) -> String {
        let encoded = rikka_open::encode_path(&self.path);
        format!(
            "[Trash Info]\nPath={encoded}\nDeletionDate={}\n",
            self.deleted
        )
    }

    pub(crate) fn parse(text: &str) -> Option<Self> {
        let mut path = None;
        let mut deleted = String::new();
        let mut inside = false;
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                inside = line == "[Trash Info]";
                continue;
            }
            if !inside {
                continue;
            }
            if let Some(value) = line.strip_prefix("Path=") {
                path = Some(rikka_open::decode_path(value));
            } else if let Some(value) = line.strip_prefix("DeletionDate=") {
                value.clone_into(&mut deleted);
            }
        }
        Some(Self {
            path: path?,
            deleted,
        })
    }

    pub(crate) fn deleted_at(&self) -> Option<SystemTime> {
        let civil: jiff::civil::DateTime = self.deleted.parse().ok()?;
        let zoned = civil.to_zoned(jiff::tz::TimeZone::system()).ok()?;
        Some(SystemTime::from(zoned.timestamp()))
    }
}

fn stamp(time: SystemTime) -> String {
    jiff::Zoned::try_from(time)
        .map(|zoned| zoned.strftime("%Y-%m-%dT%H:%M:%S").to_string())
        .unwrap_or_default()
}

pub(crate) fn info_name(name: &Path) -> OsString {
    let mut file = name.as_os_str().to_owned();
    file.push(".trashinfo");
    file
}

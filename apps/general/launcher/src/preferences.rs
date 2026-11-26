use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use launcher_install::JavaChoice;
use launcher_instance::Window;
use serde::{Deserialize, Serialize};

use crate::memory;

pub(crate) const MIN_MEMORY: u32 = 1024;
const DEFAULT_MEMORY: u32 = 4096;
const FALLBACK_TOTAL: u32 = 8192;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Preferences {
    #[serde(default)]
    pub(crate) memory_mb: Option<u32>,
    #[serde(default)]
    pub(crate) jvm_arguments: String,
    #[serde(default)]
    pub(crate) window: Option<Window>,
    #[serde(default)]
    pub(crate) java: Option<PathBuf>,
}

pub(crate) fn total_memory() -> u32 {
    static TOTAL: OnceLock<u32> = OnceLock::new();
    *TOTAL.get_or_init(|| memory::total_megabytes().unwrap_or(FALLBACK_TOTAL))
}

impl Preferences {
    pub(crate) fn load(path: &Path) -> Result<Preferences, String> {
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|error| format!("{}: {error}", path.display())),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(Preferences::default()),
            Err(error) => Err(format!("{}: {error}", path.display())),
        }
    }

    pub(crate) fn save(&self, path: &Path) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(self).map_err(|error| error.to_string())?;
        rikka_fs::replace(path, bytes).map_err(|error| format!("{}: {error}", path.display()))
    }

    pub(crate) fn memory(&self) -> u32 {
        self.memory_mb
            .unwrap_or_else(|| DEFAULT_MEMORY.min(total_memory() / 2).max(MIN_MEMORY * 2))
    }

    pub(crate) fn java_choice(&self) -> JavaChoice {
        match &self.java {
            Some(path) => JavaChoice::Custom(path.clone()),
            None => JavaChoice::Managed,
        }
    }
}

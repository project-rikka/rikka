use std::path::{Path, PathBuf};

use rikka_settings::seam::Settings;
use rikka_settings::{Key, Schema};

const DOMAIN: &str = "rikka.ade";
const RECENTS: &str = "recents";

const MAX_RECENTS: usize = 12;

#[derive(Debug)]
pub struct Config {
    settings: Option<Settings>,
    recents: Vec<PathBuf>,
}

impl Config {
    #[must_use]
    pub fn load() -> Config {
        let schema = Schema::new(DOMAIN)
            .summary("ADE")
            .key(Key::list(RECENTS, []).summary("Recently opened projects, newest first"));

        let settings = Settings::open(schema)
            .inspect_err(|error| tracing::warn!(%error, "failed to open ADE settings"))
            .ok();

        Config {
            recents: read_setting::<Vec<String>>(settings.as_ref(), RECENTS)
                .into_iter()
                .map(PathBuf::from)
                .collect(),
            settings,
        }
    }

    #[must_use]
    pub fn recents(&self) -> &[PathBuf] {
        &self.recents
    }

    pub fn add_recent(&mut self, project: &Path) {
        self.recents.retain(|recent| recent != project);
        self.recents.insert(0, project.to_owned());
        self.recents.truncate(MAX_RECENTS);
        self.store_recents();
    }

    pub fn remove_recent(&mut self, project: &Path) {
        let before = self.recents.len();
        self.recents.retain(|recent| recent != project);
        if self.recents.len() != before {
            self.store_recents();
        }
    }

    fn store_recents(&mut self) {
        let paths: Vec<String> = self
            .recents
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect();
        write_setting(self.settings.as_mut(), RECENTS, &paths);
    }
}

fn read_setting<T: serde::de::DeserializeOwned + Default>(
    settings: Option<&Settings>,
    key: &str,
) -> T {
    settings
        .and_then(|settings| settings.read(key))
        .unwrap_or_default()
}

fn write_setting(settings: Option<&mut Settings>, key: &str, value: &impl serde::Serialize) {
    if let Some(settings) = settings {
        settings.set_or_warn(key, value);
    }
}

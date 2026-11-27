use std::collections::HashMap;
use std::io;
use std::path::PathBuf;

use rikka_web::{Profile, Runtime};

use browser_model::{ProfileId, ProfileKey};

const PROFILES: &str = "browser/chrome";

#[derive(Debug, Default)]
pub(crate) struct Engines {
    persistent: HashMap<ProfileId, Runtime>,
    private: Option<Runtime>,
}

impl Engines {
    pub(crate) fn running(&self, key: ProfileKey) -> Option<Runtime> {
        let registered = match key {
            ProfileKey::Persistent(id) => self.persistent.get(&id),
            ProfileKey::Private => self.private.as_ref(),
        };
        registered.filter(|runtime| runtime.is_running()).cloned()
    }

    pub(crate) fn start(&mut self, key: ProfileKey) -> rikka_web::Result<Runtime> {
        let profile = match key {
            ProfileKey::Persistent(id) => Profile::Persistent(folder(id)?),
            ProfileKey::Private => Profile::Ephemeral,
        };
        let runtime = Runtime::builder().profile(profile).start()?;
        match key {
            ProfileKey::Persistent(id) => {
                self.persistent.insert(id, runtime.clone());
            }
            ProfileKey::Private => self.private = Some(runtime.clone()),
        }
        Ok(runtime)
    }

    pub(crate) fn forget(&mut self, profile: ProfileId) {
        self.persistent.remove(&profile);
    }

    pub(crate) fn forget_private(&mut self) {
        self.private = None;
    }
}

pub(crate) fn folder(id: ProfileId) -> rikka_web::Result<PathBuf> {
    let paths = rikka_dirs::Paths::resolve().map_err(|error| rikka_web::Error::Profile {
        path: PathBuf::from(PROFILES),
        source: io::Error::new(io::ErrorKind::NotFound, error),
    })?;
    Ok(paths.data_dir().join(PROFILES).join(id.get().to_string()))
}

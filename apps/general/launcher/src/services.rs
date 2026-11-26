use std::path::PathBuf;
use std::sync::Arc;

use launcher_content::Catalog;
use launcher_install::{Installer, Store};
use launcher_instance::{Config, ContentFile, Instance, Instances};

pub(crate) const USER_AGENT: &str = concat!(
    "project-rikka/rikka-launcher/",
    env!("CARGO_PKG_VERSION"),
    " (rikka.sh)"
);

const FOLDER: &str = "launcher";

#[derive(Debug)]
pub(crate) struct Services {
    pub(crate) data: PathBuf,
    pub(crate) config: PathBuf,
    pub(crate) instances: Instances,
    pub(crate) installer: Installer,
    pub(crate) catalog: Catalog,
}

impl Services {
    pub(crate) fn open() -> Result<Services, String> {
        let paths = rikka_dirs::Paths::resolve().map_err(|error| error.to_string())?;
        let data = paths.data_dir().join(FOLDER);
        let config = paths.config_dir().join(FOLDER);
        let installer = Installer::new(Store::new(data.join("game")), USER_AGENT)
            .map_err(|error| error.to_string())?;
        let catalog = Catalog::new(USER_AGENT).map_err(|error| error.to_string())?;
        Ok(Services {
            instances: Instances::new(data.join("instances")),
            installer,
            catalog,
            data,
            config,
        })
    }

    pub(crate) fn accounts_file(&self) -> PathBuf {
        self.config.join("accounts.json")
    }

    pub(crate) fn preferences_file(&self) -> PathBuf {
        self.config.join("preferences.json")
    }
}

pub(crate) async fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    match tokio::task::spawn_blocking(work).await {
        Ok(value) => value,
        Err(error) => std::panic::resume_unwind(error.into_panic()),
    }
}

pub(crate) async fn scan(
    services: Arc<Services>,
    id: String,
) -> launcher_instance::Result<(Instance, Vec<ContentFile>)> {
    blocking(move || {
        let mut instance = services.instances.get(&id)?;
        let files = instance.content()?;
        Ok((instance, files))
    })
    .await
}

pub(crate) async fn update(
    services: Arc<Services>,
    id: String,
    change: impl FnOnce(&mut Config) + Send + 'static,
) -> launcher_instance::Result<Instance> {
    blocking(move || services.instances.update(&id, change)).await
}

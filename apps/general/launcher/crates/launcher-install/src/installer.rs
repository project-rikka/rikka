use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use launcher_meta::{
    AssetIndex, ClientVersion, Environment, GameTarget, Platform, VERSION_MANIFEST, Version,
    VersionManifest, profile_url,
};
use rikka_net::{CancelToken, Client};

use crate::error::{Error, Result};
use crate::fetch::{Fetcher, Report, Task, Verify, blocking};
use crate::java::{self, Java, JavaChoice};
use crate::progress::{Progress, Stage};
use crate::store::Store;

const TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Installation {
    pub target: GameTarget,
    pub version: ClientVersion,
    pub classpath: Vec<PathBuf>,
    pub libraries: PathBuf,
    pub assets: PathBuf,
    pub natives: PathBuf,
    pub java: Java,
}

#[derive(Clone, Debug)]
pub struct Installer {
    http: Client,
    store: Store,
    platform: Platform,
}

impl Installer {
    pub fn new(store: Store, user_agent: &str) -> Result<Installer> {
        let http = Client::builder()
            .user_agent(user_agent)
            .timeout(TIMEOUT)
            .build()
            .map_err(|source| Error::Download {
                url: String::new(),
                source,
            })?;
        Ok(Installer {
            http,
            store,
            platform: Platform::current(),
        })
    }

    #[must_use]
    pub fn store(&self) -> &Store {
        &self.store
    }

    #[must_use]
    pub fn platform(&self) -> Platform {
        self.platform
    }

    async fn game_version(&self, fetcher: &Fetcher, minecraft: &str) -> Result<Version> {
        let path = self.store.version_json(minecraft);
        if let Ok(bytes) = std::fs::read(&path)
            && let Ok(version) = Version::from_json(&bytes)
        {
            return Ok(version);
        }
        let manifest = fetcher
            .fresh_or_cached(VERSION_MANIFEST, &self.store.manifest())
            .await?;
        let manifest = VersionManifest::from_json(&manifest)?;
        let entry = manifest
            .find(minecraft)
            .ok_or_else(|| Error::UnknownVersion(minecraft.to_owned()))?;
        let bytes = fetcher.cached(&entry.url, &path, &entry.sha1).await?;
        Ok(Version::from_json(&bytes)?)
    }

    async fn loader_version(&self, fetcher: &Fetcher, target: &GameTarget) -> Result<Version> {
        let path = self.store.version_json(&target.loader_profile());
        if let Ok(bytes) = std::fs::read(&path)
            && let Ok(version) = Version::from_json(&bytes)
        {
            return Ok(version);
        }
        let bytes = fetcher
            .bytes(&profile_url(&target.minecraft, &target.fabric_loader))
            .await?;
        let version = Version::from_json(&bytes)?;
        rikka_fs::replace(&path, &bytes).map_err(Error::io(&path))?;
        Ok(version)
    }

    pub async fn install(
        &self,
        target: &GameTarget,
        java: &JavaChoice,
        verify: Verify,
        cancel: CancelToken,
        report: impl Fn(Progress) + Send + Sync + 'static,
    ) -> Result<Installation> {
        let report: Report = Arc::new(report);
        let fetcher = Fetcher::new(self.http.clone(), cancel);
        report(Progress::starting(Stage::Metadata));
        let game = self.game_version(&fetcher, &target.minecraft).await?;
        let loader = self.loader_version(&fetcher, target).await?;
        let version = loader.inherit(game)?.into_client()?;

        report(Progress::starting(Stage::Java));
        let java = java::resolve(
            java,
            &fetcher,
            &self.store,
            self.platform,
            &version.java,
            verify,
            &report,
        )
        .await?;

        let environment = Environment::new(self.platform);
        let libraries = self.store.libraries();
        let artifacts = version.artifacts(&environment)?;
        let mut classpath: Vec<PathBuf> = artifacts
            .iter()
            .map(|artifact| artifact.path.under(&libraries))
            .collect();
        let client_jar = self.store.client_jar(&target.minecraft);
        let mut tasks: Vec<Task> = artifacts
            .into_iter()
            .map(|artifact| Task {
                path: artifact.path.under(&libraries),
                url: artifact.url,
                sha1: artifact.sha1,
                size: artifact.size,
            })
            .collect();
        tasks.push(Task {
            url: version.client.url.clone(),
            path: client_jar.clone(),
            sha1: Some(version.client.sha1.clone()),
            size: Some(version.client.size),
        });
        classpath.push(client_jar);
        fetcher
            .all(Stage::Libraries, tasks, verify, &report)
            .await?;

        let index_path = self.store.asset_index(&version.asset_index.id);
        let index = fetcher
            .cached(
                &version.asset_index.url,
                &index_path,
                &version.asset_index.sha1,
            )
            .await?;
        let objects = self.store.asset_objects();
        let tasks = blocking(move || -> Result<Vec<Task>> {
            let index = AssetIndex::from_json(&index)?;
            let mut tasks: Vec<Task> = index
                .objects
                .values()
                .map(|object| Task {
                    url: object.url(),
                    path: object.path().under(&objects),
                    sha1: Some(object.hash.clone()),
                    size: Some(object.size),
                })
                .collect();
            tasks.sort_by(|left, right| left.path.cmp(&right.path));
            tasks.dedup_by(|left, right| left.path == right.path);
            Ok(tasks)
        })
        .await?;
        fetcher.all(Stage::Assets, tasks, verify, &report).await?;
        fetcher.checked()?;

        let natives = self.store.natives(&version.id);
        std::fs::create_dir_all(&natives).map_err(Error::io(&natives))?;
        Ok(Installation {
            target: target.clone(),
            version,
            classpath,
            libraries,
            assets: self.store.assets(),
            natives,
            java,
        })
    }
}

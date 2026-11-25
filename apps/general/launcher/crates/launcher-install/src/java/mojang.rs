use launcher_meta::{
    JavaVersion, Platform, RUNTIME_INDEX, RuntimeFile, RuntimeIndex, RuntimeManifest,
};

use super::Java;
use crate::error::{Error, Result};
use crate::fetch::{Fetcher, Report, Task, Verify};
use crate::progress::Stage;
use crate::store::Store;

const EXECUTABLE: &str = "javaw.exe";

pub(super) async fn install(
    fetcher: &Fetcher,
    store: &Store,
    platform: Platform,
    version: &JavaVersion,
    verify: Verify,
    report: &Report,
) -> Result<Java> {
    let index = fetcher
        .fresh_or_cached(RUNTIME_INDEX, &store.runtime_index())
        .await?;
    let index = RuntimeIndex::from_json(&index)?;
    let release = index
        .release(platform.runtime_key(), &version.component)
        .ok_or_else(|| Error::NoRuntime {
            component: version.component.clone(),
            platform: platform.runtime_key(),
        })?;
    let manifest = fetcher
        .cached(
            &release.manifest.url,
            &store.runtime_manifest(&version.component),
            &release.manifest.sha1,
        )
        .await?;
    let manifest = RuntimeManifest::from_json(&manifest)?;
    let root = store.runtime(&version.component);
    let mut tasks = Vec::new();
    for (path, file) in &manifest.files {
        let target = path.under(&root);
        match file {
            RuntimeFile::Directory => {
                std::fs::create_dir_all(&target).map_err(Error::io(&target))?;
            }
            RuntimeFile::File { downloads, .. } => tasks.push(Task {
                url: downloads.raw.url.clone(),
                path: target,
                sha1: Some(downloads.raw.sha1.clone()),
                size: Some(downloads.raw.size),
            }),
            RuntimeFile::Link { .. } => return Err(Error::Link(path.to_string())),
        }
    }
    fetcher.all(Stage::Java, tasks, verify, report).await?;
    super::checked(
        root.join("bin").join(EXECUTABLE),
        version.major_version,
        None,
    )
}

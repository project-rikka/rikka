mod checkout;
mod github;
mod source;
mod store;
mod verify;
mod version;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

pub(crate) use self::version::{Bump, Version};

use self::checkout::Checkout;
use self::store::{State, Tagged};
use crate::bazel::{self, Profile};
use crate::components::COMPONENTS;
use crate::dist::{self, Fragment, Order, Placement, Release, SYSTEM};
use crate::host;
use crate::shipped::{Program, named};

const SOURCE: &str = "project-rikka/rikka";

const PRODUCT: &str = "Rikka";

const WORK: &str = ".bazel/release";

const KEPT_GENERATIONS: usize = 10;

const INSTALLER: Program = named("//installer:rikka-installer", "rikka-installer");

struct Work {
    checkout: PathBuf,
    archives: PathBuf,
    verify: PathBuf,
}

impl Work {
    fn new(workspace: &Path) -> Work {
        let root = workspace.join(WORK);
        Work {
            checkout: root.join("packages"),
            archives: root.join("archives"),
            verify: root.join("verify"),
        }
    }
}

pub(crate) fn release(workspace: &Path, bump: &Bump) -> Result<()> {
    source::require_published(workspace)?;
    let work = Work::new(workspace);
    let checkout = Checkout::fresh(&work.checkout)?;
    let version = bump.apply(PRODUCT, checkout.release()?.version())?;
    let tagged = Tagged::new(SOURCE, version.tag());
    match tagged.state()? {
        State::Absent => {}
        State::Draft => {
            println!("discarding the draft {tagged} that an unfinished release left");
            tagged.discard_draft()?;
        }
        State::Published => bail!("{tagged} is already released"),
    }
    let revision = dist::revision(workspace)?;

    host::reset_dir(&work.archives)?;
    let orders: Vec<Order> = COMPONENTS
        .iter()
        .map(|component| Order {
            component,
            version: version.to_string(),
        })
        .collect();
    let fragments = dist::build(workspace, &orders, &work.archives)?;
    let installer = installer(workspace, &work.archives)?;
    let windows = dist::windows(workspace, version, &work.archives)?;
    verify::verify(workspace, &fragments, &work.archives, &work.verify)?;

    let files: Vec<PathBuf> = archives_of(&fragments, &work.archives)
        .into_iter()
        .chain(installer)
        .chain(windows)
        .collect();
    checkout.ensure_started()?;
    tagged.create_draft(&revision, &format!("{PRODUCT} {version}"))?;
    tagged.upload(&files)?;
    tagged.publish_latest()?;

    let mut release = Release::empty();
    dist::merge(
        &mut release,
        &fragments,
        Placement::Downloaded {
            prefix: &tagged.download_prefix(),
        },
        checkout.root(),
    )?;
    dist::write_flake(workspace, &release, checkout.root())?;
    let message = format!("{PRODUCT} {version}\n\nFrom {SOURCE}@{revision}");
    if checkout.publish(&message)? {
        println!("published {PRODUCT} {version} as {tagged}");
    }
    prune(&checkout)
}

fn installer(workspace: &Path, out: &Path) -> Result<[PathBuf; 2]> {
    let built = bazel::build(workspace, Profile::Installer, [INSTALLER.target])?;
    let name = format!("{}-{SYSTEM}", INSTALLER.installed);
    let binary = out.join(&name);
    host::copy(built.program(&INSTALLER)?, &binary)?;
    host::set_mode(&binary, 0o755)?;
    let checksum = out.join(format!("{name}.sha256"));
    host::write(
        &checksum,
        format!("{}  {name}\n", dist::sha256_hex(&binary)?),
    )?;
    Ok([binary, checksum])
}

fn archives_of(fragments: &[Fragment], archives: &Path) -> Vec<PathBuf> {
    let files: BTreeSet<&str> = fragments
        .iter()
        .flat_map(|fragment| {
            std::iter::once(fragment.packed.file.as_str())
                .chain(fragment.assets.values().map(|packed| packed.file.as_str()))
        })
        .collect();
    files.into_iter().map(|file| archives.join(file)).collect()
}

fn prune(checkout: &Checkout) -> Result<()> {
    let mut downloads: Vec<String> = Vec::new();
    for release in checkout.history(KEPT_GENERATIONS)? {
        downloads.extend(release.downloads().map(str::to_owned));
    }
    let kept: BTreeSet<Tagged> = downloads
        .iter()
        .filter_map(|url| Tagged::of_download(url))
        .collect();
    for repository in [SOURCE, checkout::REPOSITORY] {
        for tagged in Tagged::list_published(repository)? {
            if !kept.contains(&tagged) {
                println!(
                    "removing {tagged}, which the last {KEPT_GENERATIONS} releases no longer name"
                );
                tagged.delete()?;
            }
        }
    }
    Ok(())
}

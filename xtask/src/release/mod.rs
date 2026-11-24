mod checkout;
mod gecko;
mod github;
mod source;
mod store;
mod verify;
mod version;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

pub(crate) use self::version::{Bump, Version};

use self::checkout::Checkout;
use self::store::{State, Tagged};
use crate::bazel::{self, Profile};
use crate::components::COMPONENTS;
use crate::dist::{
    self, FLAKE_SOURCES, Fragment, GECKO_PACKAGE, GeckoPin, Order, Placement, Release, SYSTEM,
};
use crate::host;
use crate::shipped::{Program, named};

const SOURCE: &str = "project-rikka/rikka";

const PRODUCT: &str = "Rikka";

const WORK: &str = ".bazel/release";

const KEPT_GENERATIONS: usize = 10;

const INSTALLER: Program = named("//installer:rikka-installer", "rikka-installer");

const GECKO_TAG_PREFIX: &str = "gecko-";

const GECKO_NOTES: &str = "The Gecko web engine that the Rikka distribution links.";

struct Work {
    checkout: PathBuf,
    archives: PathBuf,
    verify: PathBuf,
    gecko: PathBuf,
}

impl Work {
    fn new(workspace: &Path) -> Work {
        let root = workspace.join(WORK);
        Work {
            checkout: root.join("packages"),
            archives: root.join("archives"),
            verify: root.join("verify"),
            gecko: root.join("gecko"),
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
    prune(workspace, &checkout)
}

pub(crate) fn gecko(workspace: &Path) -> Result<()> {
    source::require_published(workspace)?;
    let work = Work::new(workspace);
    host::reset_dir(&work.gecko)?;
    let archive = gecko::download(workspace, &work.gecko)?;
    let file = archive
        .file_name()
        .and_then(|name| name.to_str())
        .context("the downloaded engine has a UTF-8 name")?
        .to_owned();
    let version = gecko::version_of(&file)?.to_owned();

    let checkout = Checkout::fresh(&work.checkout)?;
    let published = checkout.release()?;
    checkout.ensure_started()?;
    let tagged = Tagged::new(SOURCE, format!("{GECKO_TAG_PREFIX}{version}"));
    match tagged.state()? {
        State::Absent => tagged.create_not_latest(
            &dist::revision(workspace)?,
            &format!("Gecko {version}"),
            GECKO_NOTES,
        )?,
        State::Draft => bail!("{tagged} is a draft; publish or delete it first"),
        State::Published => {}
    }
    if !tagged.assets()?.contains(&file) {
        tagged.upload(std::slice::from_ref(&archive))?;
    }
    let pin = GeckoPin {
        version: version.clone(),
        url: tagged.download(&file),
        hash: dist::sri(&archive)?,
    };
    let flake_sources = workspace.join(FLAKE_SOURCES);
    pin.write(&flake_sources)?;
    println!("pinned Gecko {version} in {FLAKE_SOURCES}; commit it");

    let stale = gecko::stale_components(workspace, &published)?;
    if !stale.is_empty() {
        println!(
            "the Gecko protocol changed since {} were published; once the pin is committed and \
             pushed, ship the engine with the next release: bazel run //xtask -- release --patch",
            stale.join(", ")
        );
        return Ok(());
    }
    host::copy(
        &flake_sources.join(GECKO_PACKAGE),
        &checkout.root().join(GECKO_PACKAGE),
    )?;
    pin.write(checkout.root())?;
    let message = format!(
        "gecko {version}\n\nFrom {SOURCE}@{}",
        dist::revision(workspace)?
    );
    if checkout.publish(&message)? {
        println!("published Gecko {version}");
    }
    Ok(())
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

fn prune(workspace: &Path, checkout: &Checkout) -> Result<()> {
    let mut downloads: Vec<String> = Vec::new();
    for release in checkout.history(KEPT_GENERATIONS)? {
        downloads.extend(release.downloads().map(str::to_owned));
    }
    let pinned = GeckoPin::read(&workspace.join(FLAKE_SOURCES))?;
    for pin in checkout
        .gecko_history(KEPT_GENERATIONS)?
        .into_iter()
        .chain([pinned])
    {
        downloads.push(pin.url);
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

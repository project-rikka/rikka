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
use crate::bazel::{self, Profile};
use crate::components::COMPONENTS;
use crate::dist::{
    self, FLAKE_SOURCES, Fragment, GECKO_PACKAGE, GeckoPin, Installer, Order, Placement, Release,
    SYSTEM,
};
use crate::host;
use crate::shipped::{Component, Program, named};

const WORK: &str = ".bazel/release";

const KEPT_GENERATIONS: usize = 10;

const INSTALLER: Program = named("//installer:rikka-installer", "rikka-installer");

const INSTALLER_SUBJECT: &str = "installer";

#[derive(Clone, Debug)]
pub(crate) enum Selection {
    All,
    Named(Vec<String>),
}

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

pub(crate) fn components(workspace: &Path, selection: &Selection, bump: &Bump) -> Result<()> {
    source::require_published(workspace)?;
    let work = Work::new(workspace);
    let checkout = Checkout::fresh(&work.checkout)?;
    let published = checkout.release()?;

    let mut orders = Vec::new();
    for component in selected(selection)? {
        let entry = published.components.get(component.id);
        let version = bump
            .apply(component.id, entry.map(|entry| entry.version.as_str()))?
            .to_string();
        let build = match entry {
            Some(entry) if entry.version == version => entry.build + 1,
            _ => 1,
        };
        orders.push(Order {
            component,
            version,
            build,
        });
    }
    if nixpkgs_moved(workspace, &checkout)? {
        let chosen: BTreeSet<&str> = orders.iter().map(|order| order.component.id).collect();
        let rest = rebuilds(&published)?
            .into_iter()
            .filter(|order| !chosen.contains(order.component.id))
            .collect::<Vec<_>>();
        if !rest.is_empty() {
            println!(
                "nixpkgs moved since the last release; rebuilding every other component as well"
            );
        }
        orders.extend(rest);
    }
    publish(workspace, &work, &checkout, published, &orders)
}

pub(crate) fn rebuild(workspace: &Path) -> Result<()> {
    source::require_published(workspace)?;
    let work = Work::new(workspace);
    let checkout = Checkout::fresh(&work.checkout)?;
    let published = checkout.release()?;
    let orders = rebuilds(&published)?;
    if orders.is_empty() {
        bail!("nothing is published yet; release components first");
    }
    publish(workspace, &work, &checkout, published, &orders)
}

pub(crate) fn installer(workspace: &Path, bump: &Bump) -> Result<()> {
    source::require_published(workspace)?;
    let work = Work::new(workspace);
    let checkout = Checkout::fresh(&work.checkout)?;
    let mut published = checkout.release()?;
    let version = bump.apply(
        INSTALLER_SUBJECT,
        published
            .installer
            .as_ref()
            .map(|installer| installer.version.as_str()),
    )?;

    let built = bazel::build(workspace, Profile::Installer, [INSTALLER.target])?;
    host::reset_dir(&work.archives)?;
    let name = format!("{}-{SYSTEM}", INSTALLER.installed);
    let binary = work.archives.join(&name);
    host::copy(built.program(&INSTALLER)?, &binary)?;
    host::set_mode(&binary, 0o755)?;
    let checksum = work.archives.join(format!("{name}.sha256"));
    host::write(
        &checksum,
        format!("{}  {name}\n", dist::sha256_hex(&binary)?),
    )?;

    checkout.ensure_started()?;
    store::ensure()?;
    store::upload(&[binary.clone(), checksum])?;
    published.installer = Some(Installer {
        version: version.to_string(),
        revision: dist::revision(workspace)?,
        url: format!("{}/{name}", store::DOWNLOAD),
        hash: dist::sri(&binary)?,
    });
    published.write(checkout.root())?;
    let message = format!(
        "installer {version}\n\nFrom project-rikka/rikka@{}",
        dist::revision(workspace)?
    );
    if checkout.publish(&message)? {
        println!("published the installer {version}");
    }
    Ok(())
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
    store::ensure()?;
    if !store::assets()?.contains(&file) {
        store::upload(std::slice::from_ref(&archive))?;
    }
    let pin = GeckoPin {
        version: version.clone(),
        url: format!("{}/{file}", store::DOWNLOAD),
        hash: dist::sri(&archive)?,
    };
    let flake_sources = workspace.join(FLAKE_SOURCES);
    pin.write(&flake_sources)?;
    println!("pinned Gecko {version} in {FLAKE_SOURCES}; commit it");

    let stale = gecko::stale_components(workspace, &published)?;
    if !stale.is_empty() {
        println!(
            "the Gecko protocol changed since {} were published; once the pin is committed and \
             pushed, ship the engine with them: bazel run //xtask -- release components {} --patch",
            stale.join(", "),
            stale.join(" ")
        );
        return Ok(());
    }
    host::copy(
        &flake_sources.join(GECKO_PACKAGE),
        &checkout.root().join(GECKO_PACKAGE),
    )?;
    pin.write(checkout.root())?;
    let message = format!(
        "gecko {version}\n\nFrom project-rikka/rikka@{}",
        dist::revision(workspace)?
    );
    if checkout.publish(&message)? {
        println!("published Gecko {version}");
    }
    Ok(())
}

fn selected(selection: &Selection) -> Result<Vec<&'static Component>> {
    match selection {
        Selection::All => Ok(COMPONENTS.iter().collect()),
        Selection::Named(ids) => ids.iter().map(|id| dist::find(id)).collect(),
    }
}

fn rebuilds(published: &Release) -> Result<Vec<Order>> {
    published
        .components
        .iter()
        .map(|(id, entry)| {
            Ok(Order {
                component: dist::find(id)?,
                version: entry.version.clone(),
                build: entry.build + 1,
            })
        })
        .collect()
}

fn nixpkgs_moved(workspace: &Path, checkout: &Checkout) -> Result<bool> {
    let published = dist::nixpkgs_pin(&checkout.root().join(dist::LOCK))?;
    let current = dist::nixpkgs_pin(&workspace.join(dist::LOCK))?
        .context("this repository's flake.lock pins no nixpkgs")?;
    Ok(published.is_some_and(|published| published != current))
}

fn publish(
    workspace: &Path,
    work: &Work,
    checkout: &Checkout,
    published: Release,
    orders: &[Order],
) -> Result<()> {
    host::reset_dir(&work.archives)?;
    let fragments = dist::build(workspace, orders, &work.archives)?;
    verify::verify(
        workspace,
        &published,
        &fragments,
        &work.archives,
        &work.verify,
    )?;

    let present = {
        checkout.ensure_started()?;
        store::ensure()?;
        store::assets()?
    };
    store::upload(&new_archives(&fragments, &present, &work.archives))?;

    let mut release = published;
    dist::merge(
        &mut release,
        &fragments,
        Placement::Downloaded {
            prefix: store::DOWNLOAD,
        },
        checkout.root(),
    )?;
    dist::write_flake(workspace, &release, checkout.root())?;
    let summary: Vec<String> = orders
        .iter()
        .map(|order| {
            format!(
                "{} {} (build {})",
                order.component.id, order.version, order.build
            )
        })
        .collect();
    let message = format!(
        "{}\n\nFrom project-rikka/rikka@{}",
        summary.join(", "),
        dist::revision(workspace)?
    );
    if checkout.publish(&message)? {
        println!("published {}", summary.join(", "));
    }
    prune(workspace, checkout)
}

fn new_archives(
    fragments: &[Fragment],
    present: &BTreeSet<String>,
    archives: &Path,
) -> Vec<PathBuf> {
    let files: BTreeSet<&str> = fragments
        .iter()
        .flat_map(|fragment| {
            std::iter::once(fragment.packed.file.as_str())
                .chain(fragment.assets.values().map(|packed| packed.file.as_str()))
        })
        .filter(|file| !present.contains(*file))
        .collect();
    files.into_iter().map(|file| archives.join(file)).collect()
}

fn prune(workspace: &Path, checkout: &Checkout) -> Result<()> {
    let mut kept: BTreeSet<String> = BTreeSet::new();
    for release in checkout.history(KEPT_GENERATIONS)? {
        kept.extend(release.downloads().filter_map(file_of));
        if let Some(installer) = &release.installer {
            kept.extend(file_of(&installer.url));
            kept.extend(file_of(&installer.url).map(|file| format!("{file}.sha256")));
        }
    }
    let pinned = GeckoPin::read(&workspace.join(FLAKE_SOURCES))?;
    for pin in checkout
        .gecko_history(KEPT_GENERATIONS)?
        .iter()
        .chain([&pinned])
    {
        kept.extend(file_of(&pin.url));
    }
    for asset in store::assets()? {
        if !kept.contains(&asset) {
            println!("removing {asset}, which the last {KEPT_GENERATIONS} releases no longer name");
            store::delete(&asset)?;
        }
    }
    Ok(())
}

fn file_of(url: &str) -> Option<String> {
    url.strip_prefix(store::DOWNLOAD)?
        .strip_prefix('/')
        .map(str::to_owned)
}

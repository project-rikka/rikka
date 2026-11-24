use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use memchr::memmem;

use crate::bazel::{self, Profile};
use crate::components::COMPONENTS;
use crate::dist::about;
use crate::dist::archive::{ARCHIVE_SUFFIX, pack_as, pack_named};
use crate::dist::release::{Entry, Fragment, Packed, SYSTEM, Source, write_json};
use crate::host;
use crate::shipped::{Component, Contents, Distributed, PORTALS, SHARED, Shared};
use crate::tree::{self, Laid};

const PREFIX: &str = "@out@";

const STORE: &[u8] = b"/nix/store/";

const STAGE: &str = ".stage";

pub(crate) const FILE_PREFIX: &str = "rikka-";

pub(crate) struct Order {
    pub(crate) component: &'static Component,
    pub(crate) version: String,
}

pub(crate) fn find(id: &str) -> Result<&'static Component> {
    COMPONENTS
        .iter()
        .find(|component| component.id == id)
        .with_context(|| {
            let ids: Vec<&str> = COMPONENTS.iter().map(|component| component.id).collect();
            format!(
                "unknown component `{id}`; expected one of {}",
                ids.join(", ")
            )
        })
}

fn check_version(version: &str) -> Result<()> {
    let valid = version.starts_with(|first: char| first.is_ascii_digit())
        && version
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '-'));
    if !valid {
        bail!(
            "version `{version}` must start with a digit and hold only letters, digits, `.` and `-`"
        );
    }
    Ok(())
}

pub(crate) fn build(workspace: &Path, orders: &[Order], out: &Path) -> Result<Vec<Fragment>> {
    for order in orders {
        check_version(&order.version)?;
    }
    let parts: Vec<Contents> = orders
        .iter()
        .flat_map(|order| order.component.contents())
        .collect();
    let built = tree::build(workspace, Profile::Dist, &parts)?;
    let revision = revision(workspace)?;

    let stage = out.join(STAGE);
    let mut fragments = Vec::new();
    for order in orders {
        host::reset_dir(&stage)?;
        let fragment = pack_component(workspace, &built, order, &revision, &stage, out)?;
        write_json(&out.join(format!("{}.json", fragment.id)), &fragment)?;
        println!(
            "packed {} {} into {}",
            fragment.id, order.version, fragment.packed.file
        );
        fragments.push(fragment);
    }
    host::remove(&stage)?;
    Ok(fragments)
}

fn pack_component(
    workspace: &Path,
    built: &bazel::BuildOutput,
    order: &Order,
    revision: &str,
    stage: &Path,
    out: &Path,
) -> Result<Fragment> {
    let component = order.component;
    let tree = stage.join(component.id);
    let laid = tree::lay(
        workspace,
        built,
        &component.contents(),
        Path::new(PREFIX),
        &tree,
    )?;
    refuse_store_paths(&laid)?;

    let mut assets = BTreeMap::new();
    let mut links = BTreeMap::new();
    for shared in used(workspace, component)? {
        let name = match shared.distributed {
            Distributed::Flake => shared.name.to_owned(),
            Distributed::Asset => {
                let packed = pack_asset(shared, stage, out)?;
                let id = packed
                    .file
                    .strip_suffix(ARCHIVE_SUFFIX)
                    .unwrap_or(&packed.file)
                    .to_owned();
                assets.insert(id.clone(), packed);
                id
            }
        };
        links.insert(shared.into.to_owned(), name);
    }

    let file = format!(
        "{FILE_PREFIX}{}-{}-{SYSTEM}{ARCHIVE_SUFFIX}",
        component.id, order.version
    );
    let packed = pack_as(&tree, component.id, &file, out)?;
    let described = about::describe(workspace, component)?;
    let entry = Entry {
        name: described.name,
        summary: described.summary,
        offer: component.offer,
        version: order.version.clone(),
        revision: revision.to_owned(),
        pam: component
            .pam
            .iter()
            .map(|&service| service.to_owned())
            .collect(),
        portal: component
            .contents()
            .iter()
            .any(|contents| contents.data.iter().any(|data| data.into == PORTALS)),
        templates: relative(&tree, &laid.templates)?,
        links,
        source: Source::Local {
            file: packed.file.clone(),
        },
    };
    Ok(Fragment {
        id: component.id.to_owned(),
        packed,
        entry,
        assets,
    })
}

fn refuse_store_paths(laid: &Laid) -> Result<()> {
    let finder = memmem::Finder::new(STORE);
    let mut found = Vec::new();
    for file in laid.files() {
        let bytes =
            std::fs::read(file).with_context(|| format!("failed to read {}", file.display()))?;
        if finder.find(&bytes).is_some() {
            found.push(file.display().to_string());
        }
    }
    if !found.is_empty() {
        bail!(
            "distributed files must not name the Nix store, but these do:\n  {}",
            found.join("\n  ")
        );
    }
    Ok(())
}

fn used(workspace: &Path, component: &Component) -> Result<Vec<&'static Shared>> {
    let targets: Vec<&str> = component
        .contents()
        .iter()
        .flat_map(|contents| {
            contents
                .programs
                .iter()
                .map(|program| program.target)
                .chain(contents.libraries.iter().map(|library| library.target))
        })
        .collect();
    let mut used = Vec::new();
    for shared in SHARED {
        if bazel::depends_on(workspace, targets.iter().copied(), shared.library)? {
            used.push(shared);
        }
    }
    Ok(used)
}

fn pack_asset(shared: &Shared, stage: &Path, out: &Path) -> Result<Packed> {
    let folder = stage.join(shared.name);
    for source in tree::listed(&tree::supplied(shared.variable)?)? {
        let name = source.file_name().context("listed files have names")?;
        host::copy(&source, &folder.join(name))?;
    }
    pack_named(&folder, shared.name, shared.name, out)
}

fn relative(root: &Path, files: &[PathBuf]) -> Result<Vec<String>> {
    files
        .iter()
        .map(|file| {
            file.strip_prefix(root)
                .map(|path| path.to_string_lossy().into_owned())
                .with_context(|| format!("{} lies outside {}", file.display(), root.display()))
        })
        .collect()
}

pub(crate) fn revision(workspace: &Path) -> Result<String> {
    let mut command = host::clean_command(&host::require("git")?);
    command.current_dir(workspace).args(["rev-parse", "HEAD"]);
    Ok(host::output(&mut command)?.trim().to_owned())
}

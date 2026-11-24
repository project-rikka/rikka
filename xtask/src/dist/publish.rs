use std::path::Path;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use crate::dist::release::{Fragment, Packed, RELEASE, Release, Source, read_json, write_json};
use crate::host;
use crate::mirror::mirror;
use crate::tree;

pub(crate) const FLAKE_SOURCES: &str = "nix/dist";

const TEMPLATES: &str = "templates";

const LOCK: &str = "flake.lock";

const NIXPKGS: &str = "nixpkgs";

const LOCK_VERSION: u64 = 7;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Placement<'a> {
    Copied { from: &'a Path },
    Downloaded { prefix: &'a str },
}

impl Placement<'_> {
    fn place(&self, packed: &Packed, into: &Path) -> Result<Source> {
        match self {
            Self::Copied { from } => {
                host::copy(&from.join(&packed.file), &into.join(&packed.file))?;
                Ok(Source::Local {
                    file: packed.file.clone(),
                })
            }
            Self::Downloaded { prefix } => Ok(Source::Download {
                url: format!("{prefix}/{}", packed.file),
                hash: packed.hash.clone(),
            }),
        }
    }
}

pub(crate) fn publish(workspace: &Path, entries: &Path, into: &Path) -> Result<()> {
    let fragments = fragments(entries)?;
    if fragments.is_empty() {
        bail!("{} holds no packed components", entries.display());
    }
    let mut release = Release::read_or_new(&into.join(RELEASE))?;
    merge(
        &mut release,
        &fragments,
        Placement::Copied { from: entries },
        into,
    )?;
    write_flake(workspace, &release, into)?;
    println!("wrote the distribution flake into {}", into.display());
    Ok(())
}

pub(crate) fn merge(
    release: &mut Release,
    fragments: &[Fragment],
    placement: Placement<'_>,
    into: &Path,
) -> Result<()> {
    for fragment in fragments {
        for (id, packed) in &fragment.assets {
            if matches!(placement, Placement::Downloaded { .. }) && release.assets.contains_key(id)
            {
                continue;
            }
            release
                .assets
                .insert(id.clone(), placement.place(packed, into)?);
        }
        let mut entry = fragment.entry.clone();
        entry.source = placement.place(&fragment.packed, into)?;
        if let Some(replaced) = release.components.insert(fragment.id.clone(), entry) {
            forget(into, &replaced.source, &fragment.packed.file)?;
        }
    }
    prune_assets(into, release)
}

pub(crate) fn write_flake(workspace: &Path, release: &Release, into: &Path) -> Result<()> {
    release.write(into)?;
    let sources = workspace.join(FLAKE_SOURCES);
    for source in tree::listed(&sources)? {
        let name = source.file_name().context("listed files have names")?;
        host::copy(&source, &into.join(name))?;
    }
    mirror(&sources.join(TEMPLATES), &into.join(TEMPLATES), &[])?;
    write_lock(workspace, &into.join(LOCK))
}

fn fragments(entries: &Path) -> Result<Vec<Fragment>> {
    let mut fragments = Vec::new();
    for file in tree::listed(entries)? {
        if file
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            fragments.push(read_json(&file)?);
        }
    }
    Ok(fragments)
}

fn forget(into: &Path, replaced: &Source, kept: &str) -> Result<()> {
    match replaced {
        Source::Local { file } if file != kept => host::remove(&into.join(file)),
        _ => Ok(()),
    }
}

fn prune_assets(into: &Path, release: &mut Release) -> Result<()> {
    let referenced: Vec<String> = release.referenced_assets().map(str::to_owned).collect();
    let stale: Vec<(String, Source)> = release
        .assets
        .iter()
        .filter(|(id, _)| !referenced.contains(id))
        .map(|(id, source)| (id.clone(), source.clone()))
        .collect();
    for (id, source) in stale {
        release.assets.remove(&id);
        forget(into, &source, "")?;
    }
    Ok(())
}

fn write_lock(workspace: &Path, lock: &Path) -> Result<()> {
    let source = workspace.join(LOCK);
    let workspace_lock: Value = read_json(&source)?;
    let nixpkgs = workspace_lock
        .pointer(&format!("/nodes/{NIXPKGS}"))
        .with_context(|| format!("{} locks no {NIXPKGS}", source.display()))?;
    let locked = json!({
        "nodes": {
            NIXPKGS: {
                "locked": nixpkgs["locked"].clone(),
                "original": nixpkgs["original"].clone(),
            },
            "root": {
                "inputs": { NIXPKGS: NIXPKGS },
            },
        },
        "root": "root",
        "version": LOCK_VERSION,
    });
    write_json(lock, &locked)
}

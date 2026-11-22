mod mach;
mod patches;
mod tree;
mod upstream;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use self::mach::Mach;
pub(crate) use self::mach::Profile;
use self::patches::Patches;
use self::tree::Tree;
use self::upstream::Upstream;
use crate::dist::{self, SYSTEM};
use crate::host;
use crate::mirror::{copy_changed, mirror};

pub(crate) const ENGINE: &str = "engine/gecko";

pub(crate) const PACKAGE: &str = "rikka-gecko";

const WORK: &str = ".bazel/gecko";

const FIREFOX: &str = "firefox";

const STAMP: &str = "applied";

const PATCHES: &str = "patches";

const MOZCONFIG: &str = "mozconfig";

const PROJECT: &str = "rikka";

const PROJECT_CONFIG: &str = "config";

const VERSION_FILES: [&str; 2] = [
    "browser/config/version.txt",
    "browser/config/version_display.txt",
];

const REVISION_LENGTH: usize = 12;

pub(crate) struct Engine {
    workspace: PathBuf,
    source: PathBuf,
    work: PathBuf,
    tree: Tree,
    patches: Patches,
}

impl Engine {
    pub(crate) fn new(workspace: &Path, work: Option<PathBuf>) -> Engine {
        let source = workspace.join(ENGINE);
        let work = work.unwrap_or_else(|| workspace.join(WORK));
        Engine {
            workspace: workspace.to_owned(),
            tree: Tree::new(work.join(FIREFOX)),
            patches: Patches::new(source.join(PATCHES)),
            source,
            work,
        }
    }

    pub(crate) fn assemble(&self) -> Result<()> {
        let upstream = Upstream::read(&self.source)?;
        let replaced = self.tree.check_out(&upstream)?;
        let stamp = self.patches.stamp(&upstream.commit)?;
        if replaced || self.stamp()?.as_deref() != Some(stamp.as_str()) {
            eprintln!("applying the patches to Firefox {}", upstream.version);
            self.patches.apply(&self.tree)?;
            self.write_stamp(&stamp)?;
        }
        let project = self.tree.root().join(PROJECT);
        mirror(&self.source.join(PROJECT), &project, &[PROJECT_CONFIG])?;
        for file in VERSION_FILES {
            let source = self.tree.root().join(file);
            let name = source.file_name().context("version files have names")?;
            copy_changed(&source, &project.join(PROJECT_CONFIG).join(name))?;
        }
        eprintln!("assembled Gecko in {}", self.tree.root().display());
        Ok(())
    }

    pub(crate) fn build(&self, profile: Profile) -> Result<()> {
        self.built(profile).map(drop)
    }

    pub(crate) fn package(&self, profile: Profile, out: &Path) -> Result<PathBuf> {
        let mach = self.built(profile)?;
        mach.run(&["package"])?;
        let upstream = Upstream::read(&self.source)?;
        let stem = format!(
            "{PACKAGE}-{}-{}-{SYSTEM}",
            upstream.version,
            self.revision()?
        );
        std::fs::create_dir_all(out)
            .with_context(|| format!("failed to create {}", out.display()))?;
        let staged = self.objdir(profile).join("dist").join(PACKAGE);
        let packed = dist::pack_named(&staged, PACKAGE, &stem, out)?;
        let archive = out.join(packed.file);
        println!("{}", archive.display());
        Ok(archive)
    }

    pub(crate) fn export_patches(&self) -> Result<()> {
        let upstream = Upstream::read(&self.source)?;
        if self.stamp()? != Some(self.patches.stamp(&upstream.commit)?) {
            bail!(
                "{} does not hold the current patches; run `bazel run //xtask -- gecko assemble` \
                 before editing it",
                self.tree.root().display()
            );
        }
        self.patches.export(&self.tree, PROJECT, &BTreeSet::new())?;
        self.write_stamp(&self.patches.stamp(&upstream.commit)?)
    }

    pub(crate) fn follow(&self) -> Result<()> {
        let upstream = Upstream::read(&self.source)?;
        let Some(latest) = upstream.latest()? else {
            eprintln!("Firefox {} is the latest release", upstream.version);
            return Ok(());
        };
        eprintln!(
            "rebasing from Firefox {} onto {} ({})",
            upstream.version, latest.version, latest.commit
        );
        self.tree.check_out(&upstream)?;
        self.patches.apply(&self.tree)?;
        let patched = self
            .tree
            .commit_all(&format!("Rikka patches on Firefox {}", upstream.version))?;
        self.tree.move_to(&latest.commit)?;
        let conflicted = self.tree.carry(&patched)?;
        for file in &conflicted {
            eprintln!(
                "resolve {}, then run `bazel run //xtask -- gecko export-patches`",
                self.tree.root().join(file).display()
            );
        }
        let held: BTreeSet<String> = conflicted.iter().cloned().collect();
        self.patches.export(&self.tree, PROJECT, &held)?;
        latest.write(&self.source)?;
        self.write_stamp(&self.patches.stamp(&latest.commit)?)?;
        for file in conflicted {
            println!("{file}.patch");
        }
        Ok(())
    }

    fn built(&self, profile: Profile) -> Result<Mach> {
        self.assemble()?;
        let mach = Mach::new(
            self.tree.root(),
            &self.source.join(MOZCONFIG),
            &self.objdir(profile),
            profile,
        )?;
        mach.run(&["build"])?;
        Ok(mach)
    }

    fn objdir(&self, profile: Profile) -> PathBuf {
        self.work.join(format!("obj-{}", profile.name()))
    }

    fn stamp(&self) -> Result<Option<String>> {
        let path = self.work.join(STAMP);
        if !path.is_file() {
            return Ok(None);
        }
        Ok(Some(host::read(&path)?.trim().to_owned()))
    }

    fn write_stamp(&self, stamp: &str) -> Result<()> {
        host::write(&self.work.join(STAMP), format!("{stamp}\n"))
    }

    fn revision(&self) -> Result<String> {
        let mut command = host::clean_command(&host::require("git")?);
        command.current_dir(&self.workspace).args([
            "rev-parse",
            &format!("--short={REVISION_LENGTH}"),
            "HEAD",
        ]);
        Ok(host::output(&mut command)?.trim().to_owned())
    }
}

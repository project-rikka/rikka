use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::host;
use crate::shipped::Program;

const WORKSPACE: &str = "BUILD_WORKSPACE_DIRECTORY";

const WORKING_DIRECTORY: &str = "BUILD_WORKING_DIRECTORY";

const DOWNLOAD: &str = "--remote_download_toplevel";

const OUTPUTS: &str = "--starlark:expr=json.encode({\
    'target': target.label.package + ':' + target.label.name, \
    'executable': target.files_to_run.executable.path if target.files_to_run.executable else None, \
    'files': [file.path for file in target.files.to_list()]})";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Profile {
    Release,
    Dev,
    Dist,
    Installer,
    Windows,
}

impl Profile {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Release => "release",
            Self::Dev => "dev",
            Self::Dist => "dist",
            Self::Installer => "installer",
            Self::Windows => "windows",
        }
    }

    fn flags(self) -> &'static [&'static str] {
        match self {
            Self::Release => &["--config=release"],
            Self::Dev => &[],
            Self::Dist => &["--config=dist"],
            Self::Installer => &["--config=installer"],
            Self::Windows => &["--config=windows", "--config=release"],
        }
    }
}

#[derive(Debug, Deserialize)]
struct Outputs {
    target: String,
    executable: Option<PathBuf>,
    files: Vec<PathBuf>,
}

#[derive(Debug)]
pub(crate) struct BuildOutput {
    executables: BTreeMap<String, PathBuf>,
    files: BTreeMap<String, Vec<PathBuf>>,
    root: PathBuf,
}

fn key(target: &str) -> &str {
    target.trim_start_matches("//")
}

impl BuildOutput {
    pub(crate) fn program(&self, program: &Program) -> Result<&Path> {
        self.executables
            .get(key(program.target))
            .map(PathBuf::as_path)
            .with_context(|| format!("bazel built no executable for {}", program.target))
    }

    pub(crate) fn files(&self, target: &str) -> Result<&[PathBuf]> {
        self.files
            .get(key(target))
            .map(Vec::as_slice)
            .with_context(|| format!("bazel built nothing for {target}"))
    }

    pub(crate) fn only(&self, target: &str) -> Result<&Path> {
        match self.files(target)? {
            [file] => Ok(file),
            _ => bail!("bazel did not build exactly one file for {target}"),
        }
    }

    pub(crate) fn within_package(&self, target: &str) -> Result<Vec<(&Path, &Path)>> {
        let package = key(target)
            .split_once(':')
            .map_or(key(target), |(package, _)| package);
        let base = self.root.join(package);
        self.files(target)?
            .iter()
            .map(|file| {
                file.strip_prefix(&base)
                    .map(|relative| (file.as_path(), relative))
                    .with_context(|| format!("{} lies outside {package}", file.display()))
            })
            .collect()
    }
}

pub(crate) fn workspace() -> Result<PathBuf> {
    std::env::var_os(WORKSPACE)
        .filter(|workspace| !workspace.is_empty())
        .map(PathBuf::from)
        .context("cannot locate workspace; run tasks as `bazel run //xtask -- <task>`")
}

pub(crate) fn invoked_at(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path.to_owned());
    }
    let directory = std::env::var_os(WORKING_DIRECTORY)
        .filter(|directory| !directory.is_empty())
        .context("cannot resolve relative paths; run tasks as `bazel run //xtask -- <task>`")?;
    Ok(PathBuf::from(directory).join(path))
}

pub(crate) fn build<'t>(
    workspace: &Path,
    profile: Profile,
    targets: impl IntoIterator<Item = &'t str>,
) -> Result<BuildOutput> {
    build_with(workspace, profile, &[], targets)
}

pub(crate) fn build_with<'t>(
    workspace: &Path,
    profile: Profile,
    settings: &[String],
    targets: impl IntoIterator<Item = &'t str>,
) -> Result<BuildOutput> {
    let targets: BTreeSet<&str> = targets.into_iter().collect();
    let targets: Vec<&str> = targets.into_iter().collect();

    host::run(
        bazel(workspace)?
            .arg("build")
            .args(profile.flags())
            .args(settings)
            .arg(DOWNLOAD)
            .args(&targets),
    )?;

    let root = PathBuf::from(
        host::output(
            bazel(workspace)?
                .arg("info")
                .args(profile.flags())
                .args(settings)
                .arg("execution_root"),
        )?
        .trim(),
    );
    let printed = host::output(
        bazel(workspace)?
            .arg("cquery")
            .args(profile.flags())
            .args(settings)
            .args(["--output=starlark", OUTPUTS])
            .arg(targets.join(" + ")),
    )?;
    let mut built = BuildOutput {
        executables: BTreeMap::new(),
        files: BTreeMap::new(),
        root: root.clone(),
    };
    for line in printed.lines().filter(|line| !line.trim().is_empty()) {
        let outputs: Outputs = serde_json::from_str(line)
            .with_context(|| format!("bazel cquery printed an unexpected line: {line}"))?;
        if let Some(executable) = outputs.executable {
            built
                .executables
                .insert(outputs.target.clone(), root.join(executable));
        }
        let files = outputs.files.iter().map(|file| root.join(file)).collect();
        built.files.insert(outputs.target, files);
    }
    Ok(built)
}

pub(crate) fn depends_on<'t>(
    workspace: &Path,
    targets: impl IntoIterator<Item = &'t str>,
    library: &str,
) -> Result<bool> {
    let targets: Vec<&str> = targets.into_iter().collect();
    if targets.is_empty() {
        return Ok(false);
    }
    let printed = host::output(
        bazel(workspace)?
            .arg("query")
            .arg("--output=label")
            .arg(format!("somepath(set({}), {library})", targets.join(" "))),
    )?;
    Ok(printed.lines().any(|line| !line.trim().is_empty()))
}

fn bazel(workspace: &Path) -> Result<Command> {
    let mut command = Command::new(host::require("bazel")?);
    command.current_dir(workspace);
    Ok(command)
}

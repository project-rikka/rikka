use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::bazel::{self, BuildOutput, Profile};
use crate::host;
use crate::shipped::{
    Contents, MATERIAL_FONT, MATERIAL_LISTING, SHARED, SYMBOLIC_ICON_DIRECTORY, Supplied,
};

pub(crate) const BIN: &str = "bin";

const LIB: &str = "lib";

const BINDIR: &str = "@BINDIR@";

const PREFIX: &str = "@PREFIX@";

const TEMPLATE_SUFFIX: &str = ".in";

const SYMBOLIC_SUFFIX: &str = "-symbolic.svg";

#[derive(Debug, Default)]
pub(crate) struct Laid {
    pub(crate) programs: Vec<PathBuf>,
    pub(crate) data: Vec<PathBuf>,
    pub(crate) templates: Vec<PathBuf>,
}

impl Laid {
    pub(crate) fn files(&self) -> impl Iterator<Item = &Path> {
        self.programs
            .iter()
            .chain(&self.data)
            .chain(&self.templates)
            .map(PathBuf::as_path)
    }
}

pub(crate) fn build(workspace: &Path, profile: Profile, parts: &[Contents]) -> Result<BuildOutput> {
    let programs = parts
        .iter()
        .flat_map(|part| part.programs)
        .map(|program| program.target);
    let completions = parts
        .iter()
        .flat_map(|part| part.completions)
        .map(|completion| completion.target);
    let libraries = parts
        .iter()
        .flat_map(|part| part.libraries)
        .map(|library| library.target);
    let packages = parts
        .iter()
        .flat_map(|part| part.packages)
        .map(|package| package.target);
    bazel::build(
        workspace,
        profile,
        programs.chain(completions).chain(libraries).chain(packages),
    )
}

pub(crate) fn lay(
    workspace: &Path,
    built: &BuildOutput,
    parts: &[Contents],
    prefix: &Path,
    root: &Path,
) -> Result<Laid> {
    let mut laid = Laid::default();
    for part in parts {
        laid.programs.extend(programs_of(part, built, root)?);
        laid.data.extend(libraries_of(part, built, root)?);
        laid.data.extend(data_of(part, workspace, root)?);
        laid.templates
            .extend(templates_of(part, workspace, prefix, root)?);
        laid.data.extend(icons_of(part, root)?);
        laid.data.extend(completions_of(part, built, root)?);
        laid.data.extend(packages_of(part, built, root)?);
    }
    Ok(laid)
}

pub(crate) fn link_shared(root: &Path) -> Result<()> {
    for shared in SHARED {
        host::symlink(&supplied(shared.variable)?, &root.join(shared.into))?;
    }
    Ok(())
}

pub(crate) fn supplied(variable: &str) -> Result<PathBuf> {
    std::env::var_os(variable)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .with_context(|| format!("{variable} is not set; run inside `nix develop`"))
}

fn programs_of(part: &Contents, built: &BuildOutput, root: &Path) -> Result<Vec<PathBuf>> {
    let mut laid = Vec::new();
    for program in part.programs {
        let install = root.join(BIN).join(program.installed);
        host::copy(built.program(program)?, &install)?;
        host::set_mode(&install, 0o755)?;
        laid.push(install);
    }
    Ok(laid)
}

fn libraries_of(part: &Contents, built: &BuildOutput, root: &Path) -> Result<Vec<PathBuf>> {
    let mut laid = Vec::new();
    for library in part.libraries {
        let built_library = built.only(library.target)?;
        let installed = file_name(built_library)?;
        let install = root.join(LIB).join(&installed);
        host::copy(built_library, &install)?;
        host::set_mode(&install, 0o755)?;
        host::symlink(Path::new(&installed), &root.join(LIB).join(library.link))?;
        laid.push(install);
    }
    Ok(laid)
}

fn data_of(part: &Contents, workspace: &Path, root: &Path) -> Result<Vec<PathBuf>> {
    let mut laid = Vec::new();
    for data in part.data {
        let source = workspace.join(data.source);
        let install = root.join(data.into).join(file_name(&source)?);
        host::copy(&source, &install)?;
        host::set_mode(&install, 0o644)?;
        laid.push(install);
    }
    Ok(laid)
}

fn templates_of(
    part: &Contents,
    workspace: &Path,
    prefix: &Path,
    root: &Path,
) -> Result<Vec<PathBuf>> {
    let bindir = prefix.join(BIN);
    let mut laid = Vec::new();
    for template in part.templates {
        let source = workspace.join(template.source);
        let name = file_name(&source)?;
        let Some(filled) = name.strip_suffix(TEMPLATE_SUFFIX) else {
            bail!(
                "template {} does not end in {TEMPLATE_SUFFIX}",
                source.display()
            );
        };
        let install = root.join(template.into).join(filled);
        let text = host::read(&source)?;
        let contents = text
            .replace(BINDIR, &bindir.to_string_lossy())
            .replace(PREFIX, &prefix.to_string_lossy());
        host::write(&install, contents)?;
        host::set_mode(&install, 0o644)?;
        laid.push(install);
    }
    Ok(laid)
}

fn icons_of(part: &Contents, root: &Path) -> Result<Vec<PathBuf>> {
    if part.icons.is_empty() {
        return Ok(Vec::new());
    }
    let listing = supplied_file(MATERIAL_LISTING)?;
    let font = supplied_file(MATERIAL_FONT)?;
    let mut laid = Vec::new();
    for icon in part.icons {
        let drawn = rikka_symbols_scanner::symbolic_icon(&listing, &font, icon.symbol)
            .with_context(|| format!("failed to render symbolic icon of {}", icon.application))?;
        let install = root
            .join(SYMBOLIC_ICON_DIRECTORY)
            .join(format!("{}{SYMBOLIC_SUFFIX}", icon.application));
        host::write(&install, &drawn)?;
        host::set_mode(&install, 0o644)?;
        laid.push(install);
    }
    Ok(laid)
}

fn completions_of(part: &Contents, built: &BuildOutput, root: &Path) -> Result<Vec<PathBuf>> {
    let mut laid = Vec::new();
    for completion in part.completions {
        let script = built.only(completion.target)?;
        let install = root.join(completion.into).join(file_name(script)?);
        host::copy(script, &install)?;
        host::set_mode(&install, 0o644)?;
        laid.push(install);
    }
    Ok(laid)
}

fn supplied_file(supplied_by: Supplied) -> Result<PathBuf> {
    Ok(supplied(supplied_by.variable)?.join(supplied_by.file))
}

fn packages_of(part: &Contents, built: &BuildOutput, root: &Path) -> Result<Vec<PathBuf>> {
    let mut laid = Vec::new();
    for package in part.packages {
        for (file, relative) in built.within_package(package.target)? {
            let install = root.join(package.into).join(relative);
            host::copy(file, &install)?;
            host::set_mode(&install, 0o644)?;
            laid.push(install);
        }
    }
    Ok(laid)
}

pub(crate) fn listed(folder: &Path) -> Result<Vec<PathBuf>> {
    let entries = std::fs::read_dir(folder)
        .with_context(|| format!("failed to read {}", folder.display()))?;
    let mut listed = Vec::new();
    for entry in entries {
        let path = entry
            .with_context(|| format!("failed to read {}", folder.display()))?
            .path();
        if path.is_file() {
            listed.push(path);
        }
    }
    listed.sort();
    Ok(listed)
}

fn file_name(path: &Path) -> Result<String> {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .with_context(|| format!("{} names no file", path.display()))
}

mod constant;
mod declarations;
mod dependencies;
mod entry;
mod error;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

pub use crate::declarations::{Bundled, Module, Objects, declarations, write_declarations};
pub use crate::dependencies::unused_modules;
pub use crate::error::{Error, Result};

use crate::entry::{EntryFault, EntryPoint, Stage, entry_points};

const SLANG_FLAGS: [&str; 11] = [
    "-target",
    "spirv",
    "-profile",
    rikka_spirv::SLANG_PROFILE,
    "-fvk-use-scalar-layout",
    "-matrix-layout-column-major",
    "-O2",
    "-warnings-disable",
    rikka_spirv::SLANG_QUIET_WARNINGS,
    "-warnings-as-errors",
    rikka_spirv::SLANG_WARNINGS_AS_ERRORS,
];

const VALIDATION: (&str, &str) = ("SLANG_RUN_SPIRV_VALIDATION", "1");

const SPIRV_EXTENSION: &str = "spv";

const DEPENDENCIES_EXTENSION: &str = "d";

#[derive(Clone, Copy, Debug, Default)]
enum MemoryModel {
    #[default]
    Glsl450,
    Vulkan,
}

impl MemoryModel {
    const fn flags(self) -> &'static [&'static str] {
        match self {
            Self::Glsl450 => &[],
            Self::Vulkan => &["-capability", "vk_mem_model"],
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
enum FloatingPoint {
    #[default]
    Contractible,
    Precise,
}

impl FloatingPoint {
    const fn flags(self) -> &'static [&'static str] {
        match self {
            Self::Contractible => &[],
            Self::Precise => &["-fp-mode", "precise"],
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Compiled {
    pub objects: Vec<PathBuf>,
    pub sources: BTreeSet<PathBuf>,
}

#[derive(Debug)]
#[must_use]
pub struct Compiler {
    slangc: PathBuf,
    includes: Vec<PathBuf>,
    memory_model: MemoryModel,
    floating_point: FloatingPoint,
}

struct Running {
    entry: EntryPoint,
    object: PathBuf,
    dependencies: PathBuf,
    child: Child,
}

impl Compiler {
    pub fn new(slangc: impl Into<PathBuf>) -> Self {
        Self {
            slangc: slangc.into(),
            includes: Vec::new(),
            memory_model: MemoryModel::default(),
            floating_point: FloatingPoint::default(),
        }
    }

    pub fn include(mut self, directory: impl Into<PathBuf>) -> Self {
        self.includes.push(directory.into());
        self
    }

    pub fn vulkan_memory_model(mut self) -> Self {
        self.memory_model = MemoryModel::Vulkan;
        self
    }

    pub fn precise_floating_point(mut self) -> Self {
        self.floating_point = FloatingPoint::Precise;
        self
    }

    pub fn includes(&self) -> &[PathBuf] {
        &self.includes
    }

    pub fn object(&self, source: &Path, output: &Path) -> Result<Compiled> {
        let entries = slang_entry_points(source)?;
        check_one_per_stage(source, &entries)?;
        std::fs::create_dir_all(output).map_err(|error| Error::Write {
            path: output.to_owned(),
            source: error,
        })?;
        let stem = file_stem(source);
        let running: Vec<Running> = entries
            .into_iter()
            .map(|entry| {
                let named = |extension: &str| {
                    output.join(format!("{stem}.{}.{extension}", entry.stage.extension()))
                };
                let (object, dependencies) =
                    (named(SPIRV_EXTENSION), named(DEPENDENCIES_EXTENSION));
                let child = self.start(source, &entry, &object, &dependencies)?;
                Ok(Running {
                    entry,
                    object,
                    dependencies,
                    child,
                })
            })
            .collect::<Result<_>>()?;
        let mut compiled = Compiled::default();
        for run in running {
            let ran = run.child.wait_with_output().map_err(|error| Error::Spawn {
                compiler: self.slangc.clone(),
                source: error,
            })?;
            if !ran.status.success() {
                return Err(Error::Compile {
                    shader: source.to_owned(),
                    report: String::from_utf8_lossy(&ran.stderr).trim().to_owned(),
                });
            }
            if run.entry.stage == Stage::Vertex {
                invariant_position(source, &run.object)?;
            }
            compiled
                .sources
                .extend(dependencies::taken(&run.dependencies)?);
            compiled.objects.push(run.object);
        }
        Ok(compiled)
    }

    fn start(
        &self,
        source: &Path,
        entry: &EntryPoint,
        object: &Path,
        dependencies: &Path,
    ) -> Result<Child> {
        let mut command = Command::new(&self.slangc);
        command
            .args(SLANG_FLAGS)
            .args(self.memory_model.flags())
            .args(self.floating_point.flags())
            .env(VALIDATION.0, VALIDATION.1);
        for directory in &self.includes {
            command.arg("-I").arg(directory);
        }
        command
            .arg("-entry")
            .arg(&entry.name)
            .arg("-o")
            .arg(object)
            .arg("-depfile")
            .arg(dependencies)
            .arg(source)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| Error::Spawn {
                compiler: self.slangc.clone(),
                source: error,
            })
    }
}

pub(crate) fn read(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).map_err(|source| Error::Read {
        path: path.to_owned(),
        source,
    })
}

pub(crate) fn write(path: &Path, contents: impl AsRef<[u8]>) -> Result<()> {
    std::fs::write(path, contents).map_err(|source| Error::Write {
        path: path.to_owned(),
        source,
    })
}

pub(crate) fn source_entry_points(path: &Path, text: &str) -> Result<Vec<EntryPoint>> {
    entry_points(text).map_err(|fault| match fault {
        EntryFault::UnknownStage(stage) => Error::UnknownStage {
            shader: path.to_owned(),
            stage,
        },
        EntryFault::Unnamed(stage) => Error::UnnamedEntryPoint {
            shader: path.to_owned(),
            stage: stage.extension(),
        },
    })
}

fn slang_entry_points(path: &Path) -> Result<Vec<EntryPoint>> {
    let entries = source_entry_points(path, &read(path)?)?;
    if entries.is_empty() {
        return Err(Error::MissingEntryPoint(path.to_owned()));
    }
    Ok(entries)
}

fn check_one_per_stage(path: &Path, entries: &[EntryPoint]) -> Result<()> {
    let mut stages = BTreeMap::new();
    for entry in entries {
        if let Some(first) = stages.insert(entry.stage, &entry.name) {
            return Err(Error::DuplicateStage {
                shader: path.to_owned(),
                stage: entry.stage.extension(),
                first: first.clone(),
                second: entry.name.clone(),
            });
        }
    }
    Ok(())
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn invariant_position(shader: &Path, object: &Path) -> Result<()> {
    let spirv = std::fs::read(object).map_err(|source| Error::Read {
        path: object.to_owned(),
        source,
    })?;
    let mut words = rikka_spirv::words(&spirv).map_err(|source| Error::Spirv {
        path: object.to_owned(),
        source,
    })?;
    if !rikka_spirv::invariant_position(&mut words) {
        return Err(Error::MissingPosition(shader.to_owned()));
    }
    write(object, rikka_spirv::bytes(&words))
}

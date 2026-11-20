use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::{Path, PathBuf};

use rikka_spirv::Facts;

use crate::constant::constant;
use crate::error::{Error, Result};
use crate::{SPIRV_EXTENSION, read, source_entry_points, write};

const LIBRARY: &str = "SLANG_LIBRARY";

#[derive(Clone, Debug)]
pub struct Objects {
    pub directory: PathBuf,
    pub included: String,
}

#[derive(Clone, Debug)]
pub struct Bundled {
    pub name: String,
    pub included: String,
}

#[derive(Clone, Debug)]
pub struct Module {
    pub path: PathBuf,
    pub bundled: Bundled,
}

pub fn declarations(objects: &[Objects], library: &[Module]) -> Result<String> {
    let mut named: BTreeMap<String, (PathBuf, String)> = BTreeMap::new();
    for set in objects {
        for object in compiled(&set.directory)? {
            let file = object
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            let stem = file
                .strip_suffix(&format!(".{SPIRV_EXTENSION}"))
                .unwrap_or(&file);
            let name = constant(stem);
            let included = format!("{}/{file}", set.included);
            if let Some((first, _)) = named.insert(name.clone(), (object.clone(), included)) {
                return Err(Error::ConstantCollision {
                    constant: name,
                    first,
                    second: object,
                });
            }
        }
    }
    for module in library {
        if !source_entry_points(&module.path, &read(&module.path)?)?.is_empty() {
            return Err(Error::LibraryEntryPoint(module.path.clone()));
        }
    }
    let mut text = String::new();
    for (name, (object, included)) in &named {
        writeln!(
            text,
            "pub static {name}: rikka_spirv::Shader = {};",
            facts(object)?.declaration(included)
        )
        .expect("writing to a String");
    }
    bundle(
        &mut text,
        LIBRARY,
        library.iter().map(|module| &module.bundled),
    );
    Ok(text)
}

pub fn write_declarations(output: &Path, objects: &[Objects], library: &[Module]) -> Result<()> {
    write(output, declarations(objects, library)?)
}

fn facts(object: &Path) -> Result<Facts> {
    let spirv = std::fs::read(object).map_err(|source| Error::Read {
        path: object.to_owned(),
        source,
    })?;
    Facts::of(&spirv).map_err(|source| Error::Spirv {
        path: object.to_owned(),
        source,
    })
}

fn compiled(directory: &Path) -> Result<Vec<PathBuf>> {
    let entries = std::fs::read_dir(directory).map_err(|source| Error::Read {
        path: directory.to_owned(),
        source,
    })?;
    let mut found = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|source| Error::Read {
                path: directory.to_owned(),
                source,
            })?
            .path();
        if path
            .extension()
            .is_some_and(|extension| extension == SPIRV_EXTENSION)
        {
            found.push(path);
        }
    }
    found.sort();
    Ok(found)
}

fn bundle<'a>(text: &mut String, name: &str, modules: impl Iterator<Item = &'a Bundled>) {
    let mut sorted: Vec<&Bundled> = modules.collect();
    if sorted.is_empty() {
        return;
    }
    sorted.sort_by(|left, right| left.name.cmp(&right.name));
    writeln!(text, "pub static {name}: &[(&str, &str)] = &[").expect("writing to a String");
    for module in sorted {
        writeln!(
            text,
            "    ({:?}, include_str!({:?})),",
            module.name, module.included
        )
        .expect("writing to a String");
    }
    writeln!(text, "];").expect("writing to a String");
}

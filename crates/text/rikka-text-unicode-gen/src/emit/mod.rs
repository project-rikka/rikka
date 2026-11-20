mod bidi;
mod dictionaries;
mod enums;
mod layout;
mod line_break;
mod normalization;
mod properties;
mod scripts;
mod shaping;
mod trie;

use crate::dictionary::Dictionaries;
use crate::error::Result;
use crate::line_break::LineBreaking;
use crate::model::Model;

#[derive(Debug)]
pub(crate) struct Generated {
    pub(crate) name: String,
    pub(crate) contents: Vec<u8>,
}

impl Generated {
    fn source(name: &str, contents: String) -> Self {
        Self {
            name: name.to_owned(),
            contents: contents.into_bytes(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct Report {
    pub(crate) files: Vec<Generated>,
    pub(crate) sizes: Vec<(&'static str, usize)>,
}

pub(crate) fn generate(
    model: &Model,
    dictionaries: &Dictionaries,
    version: (u32, u32, u32),
) -> Result<Report> {
    let mut sizes = Vec::new();
    let breaking = LineBreaking::derive(model)?;
    let mut files = vec![
        Generated::source("mod.rs", module_list(version)),
        Generated::source("enums.rs", enums::emit(model)),
        Generated::source("scripts.rs", scripts::emit(model, &mut sizes)),
        Generated::source(
            "properties.rs",
            properties::emit(model, &breaking.columns, &mut sizes)?,
        ),
        Generated::source("shaping.rs", shaping::emit(model, &mut sizes)?),
        Generated::source("normalization.rs", normalization::emit(model, &mut sizes)?),
        Generated::source("bidi.rs", bidi::emit(model, &mut sizes)),
        Generated::source("line_break.rs", line_break::emit(&breaking, &mut sizes)?),
        Generated::source(
            "dictionaries.rs",
            dictionaries::emit(dictionaries, &mut sizes),
        ),
    ];
    files.extend(dictionaries.complex.iter().map(|dictionary| Generated {
        name: dictionaries::file_name(dictionary),
        contents: dictionaries::blob(dictionary),
    }));
    files.push(Generated {
        name: dictionaries::cj_file_name(),
        contents: dictionaries.cj.blob(),
    });
    Ok(Report { files, sizes })
}

fn module_list(version: (u32, u32, u32)) -> String {
    let (major, minor, update) = version;
    let mut source = crate::rust::RustSource::default();
    for module in [
        "bidi",
        "dictionaries",
        "enums",
        "line_break",
        "normalization",
        "properties",
        "scripts",
        "shaping",
    ] {
        source.line(format!("pub(crate) mod {module};"));
    }
    source.blank();
    source.line(format!(
        "pub(crate) const UNICODE_VERSION: (u8, u8, u8) = ({major}, {minor}, {update});"
    ));
    source.finish()
}

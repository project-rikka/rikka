use crate::error::{Error, Result};
use crate::parameters::{Parameters, knobs};
use rikka_image_edit::{self as imaging, Operation};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(from = "LoadedTable", into = "LoadedTable")]
pub struct ColorLookup {
    table: Option<Table>,
}

#[derive(Clone, Debug)]
struct Table {
    name: String,
    source: Arc<str>,
    parsed: imaging::ColorLookup,
}

impl PartialEq for Table {
    fn eq(&self, other: &Table) -> bool {
        self.name == other.name
            && (Arc::ptr_eq(&self.source, &other.source) || self.source == other.source)
    }
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct LoadedTable {
    name: String,
    source: String,
}

impl From<LoadedTable> for ColorLookup {
    fn from(stored: LoadedTable) -> ColorLookup {
        ColorLookup::parse(stored.name, &stored.source).unwrap_or_default()
    }
}

impl From<ColorLookup> for LoadedTable {
    fn from(lookup: ColorLookup) -> LoadedTable {
        lookup
            .table
            .map_or_else(LoadedTable::default, |table| LoadedTable {
                name: table.name,
                source: table.source.to_string(),
            })
    }
}

impl ColorLookup {
    pub fn parse(name: impl Into<String>, source: &str) -> Result<ColorLookup> {
        let parsed = imaging::ColorLookup::parse(source).map_err(|_| Error::NotALookupTable)?;
        Ok(ColorLookup {
            table: Some(Table {
                name: name.into(),
                source: Arc::from(source),
                parsed,
            }),
        })
    }

    pub fn read(path: &Path) -> Result<ColorLookup> {
        let source = std::fs::read_to_string(path).map_err(|source| Error::Unreadable {
            path: path.to_path_buf(),
            source,
        })?;
        let name = path.file_name().map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        ColorLookup::parse(name, &source)
    }

    pub fn name(&self) -> Option<&str> {
        self.table.as_ref().map(|table| table.name.as_str())
    }
}

knobs! {
    for ColorLookup;
}

impl Parameters for ColorLookup {
    fn operation(&self) -> Option<Operation> {
        Some(self.table.as_ref()?.parsed.clone().into())
    }
}

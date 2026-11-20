use std::io::Read;
use std::path::PathBuf;

use rikka_portal::Result;
use rikka_types::WindowIdentifier;
use serde::{Deserialize, Serialize};

use crate::choice::Choice;
use crate::filter::Filter;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Open,
    Save,
    SaveInto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Ask {
    pub kind: Kind,
    #[serde(default)]
    pub title: String,
    #[serde(default, with = "crate::wire::maybe_identifier")]
    pub parent: Option<WindowIdentifier>,
    #[serde(default = "modal_by_default")]
    pub modal: bool,
    #[serde(default)]
    pub accept_label: String,
    #[serde(default, with = "crate::wire::maybe_path")]
    pub folder: Option<PathBuf>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub names: Vec<String>,
    #[serde(default)]
    pub filters: Vec<Filter>,
    #[serde(default)]
    pub current_filter: Option<usize>,
    #[serde(default)]
    pub choices: Vec<Choice>,
    #[serde(default)]
    pub multiple: bool,
    #[serde(default)]
    pub directory: bool,
}

impl Ask {
    #[must_use]
    pub fn new(kind: Kind) -> Self {
        Self {
            kind,
            title: String::new(),
            parent: None,
            modal: modal_by_default(),
            accept_label: String::new(),
            folder: None,
            name: String::new(),
            names: Vec::new(),
            filters: Vec::new(),
            current_filter: None,
            choices: Vec::new(),
            multiple: false,
            directory: false,
        }
    }

    pub fn read(reader: impl Read) -> Result<Self> {
        rikka_portal::read(reader)
    }
}

fn modal_by_default() -> bool {
    true
}

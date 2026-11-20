use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::dist::release::{read_json, write_json};

pub(crate) const GECKO_PIN: &str = "gecko.json";

pub(crate) const GECKO_PACKAGE: &str = "gecko.nix";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct GeckoPin {
    pub(crate) version: String,
    pub(crate) url: String,
    pub(crate) hash: String,
}

impl GeckoPin {
    pub(crate) fn read(folder: &Path) -> Result<GeckoPin> {
        read_json(&folder.join(GECKO_PIN))
    }

    pub(crate) fn parse(text: &str) -> Result<GeckoPin> {
        serde_json::from_str(text).with_context(|| format!("failed to parse {GECKO_PIN}"))
    }

    pub(crate) fn write(&self, folder: &Path) -> Result<()> {
        write_json(&folder.join(GECKO_PIN), self)
    }
}

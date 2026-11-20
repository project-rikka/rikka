use ade_scope::{Fields, Text, matter};
use serde::Deserialize;

use crate::error::Result;

#[derive(Debug, Default, Deserialize)]
pub(crate) struct Front {
    #[serde(default)]
    pub(crate) name: Text,
    #[serde(default)]
    pub(crate) description: Text,
    #[serde(default)]
    pub(crate) license: Text,
    #[serde(default)]
    pub(crate) compatibility: Text,
    #[serde(default)]
    pub(crate) metadata: Fields,
    #[serde(default, rename = "allowed-tools")]
    pub(crate) allowed_tools: Text,
}

impl Front {
    pub(crate) fn read(text: &str) -> Result<(Front, &str)> {
        matter::read(text).map_err(Into::into)
    }
}

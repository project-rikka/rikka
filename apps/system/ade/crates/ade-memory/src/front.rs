use ade_scope::{List, Text, matter};
use serde::Deserialize;

use crate::error::Result;

#[derive(Debug, Default, Deserialize)]
pub(crate) struct Front {
    #[serde(default)]
    pub(crate) title: Text,
    #[serde(default)]
    pub(crate) description: Text,
    #[serde(default)]
    pub(crate) about: Text,
    #[serde(default)]
    pub(crate) tags: List,
}

impl Front {
    pub(crate) fn read(text: &str) -> Result<(Front, &str)> {
        matter::read(text).map_err(Into::into)
    }
}

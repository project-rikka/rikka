use ade_scope::{FrontMatterError, Text, matter};
use serde::Deserialize;

use crate::error::Result;

#[derive(Debug, Default, Deserialize)]
pub(crate) struct Front {
    #[serde(default)]
    pub(crate) description: Text,
    #[serde(default, rename = "argument-hint")]
    pub(crate) argument_hint: Text,
}

impl Front {
    pub(crate) fn read(text: &str) -> Result<(Front, &str)> {
        match matter::read(text) {
            Ok(read) => Ok(read),
            Err(FrontMatterError::Missing) => Ok((Front::default(), bare(text))),
            Err(error) => Err(error.into()),
        }
    }
}

fn bare(text: &str) -> &str {
    text.trim_start_matches('\u{feff}').trim()
}

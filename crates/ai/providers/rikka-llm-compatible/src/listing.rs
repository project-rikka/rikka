use std::collections::BTreeSet;

use rikka_llm::{Error, Result};
use serde_json::Value;

use crate::offered::Offered;

const LISTS: [&str; 2] = ["data", "models"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Listing {
    Models(Vec<Offered>),
    Unlisted,
}

impl Listing {
    #[must_use]
    pub fn models(&self) -> &[Offered] {
        match self {
            Listing::Models(models) => models,
            Listing::Unlisted => &[],
        }
    }
}

pub(crate) fn read(body: &Value) -> Result<Listing> {
    let entries = LISTS
        .iter()
        .find_map(|field| body.get(field).and_then(Value::as_array))
        .or_else(|| body.as_array())
        .ok_or_else(|| Error::Malformed("the model list is not a list".to_owned()))?;

    let mut seen = BTreeSet::new();
    Ok(Listing::Models(
        entries
            .iter()
            .filter_map(Offered::read)
            .filter(|offered| seen.insert(offered.id().to_owned()))
            .collect(),
    ))
}

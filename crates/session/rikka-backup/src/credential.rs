use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::origin::Origin;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Credential {
    pub label: String,
    pub attributes: BTreeMap<String, String>,
    #[serde(with = "crate::bytes")]
    pub secret: Zeroizing<Vec<u8>>,
}

impl Credential {
    #[must_use]
    pub fn rebased(&self, origin: &Origin, home: &str) -> BTreeMap<String, String> {
        self.attributes
            .iter()
            .map(|(name, value)| (name.clone(), origin.rebase(value, home)))
            .collect()
    }
}

impl std::fmt::Debug for Credential {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Credential")
            .field("label", &self.label)
            .field("attributes", &self.attributes)
            .finish_non_exhaustive()
    }
}

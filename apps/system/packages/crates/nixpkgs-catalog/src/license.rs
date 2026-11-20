use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct License {
    pub name: String,
    pub spdx: String,
    pub url: String,
}

impl License {
    #[must_use]
    pub fn label(&self) -> &str {
        match self.spdx.is_empty() {
            true => &self.name,
            false => &self.spdx,
        }
    }
}

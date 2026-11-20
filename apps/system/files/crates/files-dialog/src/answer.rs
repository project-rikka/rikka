use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Answer {
    #[serde(with = "crate::wire::paths")]
    pub chosen: Vec<PathBuf>,
    #[serde(default)]
    pub filter: Option<usize>,
    #[serde(default)]
    pub choices: Vec<(String, String)>,
    #[serde(default)]
    pub writable: bool,
}

impl Answer {
    #[must_use]
    pub fn new(chosen: Vec<PathBuf>) -> Self {
        Self {
            chosen,
            filter: None,
            choices: Vec::new(),
            writable: false,
        }
    }
}

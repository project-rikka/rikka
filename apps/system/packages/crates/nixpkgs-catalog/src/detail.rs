use serde::{Deserialize, Serialize};

use crate::license::License;
use crate::maintainer::Maintainer;
use crate::team::Team;

#[derive(Debug, Default, Deserialize, Serialize)]
pub(crate) struct Detail {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) about: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) main_program: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) outputs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) platforms: Vec<u16>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) licenses: Vec<License>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) maintainers: Vec<Maintainer>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) teams: Vec<Team>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) homepages: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) position: String,
}

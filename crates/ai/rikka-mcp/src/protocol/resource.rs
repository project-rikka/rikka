use serde::{Deserialize, Serialize};

use crate::protocol::content::ResourceContents;
use crate::protocol::method;
use crate::protocol::page::Listable;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Resource {
    pub(crate) uri: String,
    #[serde(default)]
    pub(crate) name: String,
    pub(crate) title: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) mime_type: Option<String>,
    pub(crate) size: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ResourceTemplate {
    pub(crate) uri_template: String,
    #[serde(default)]
    pub(crate) name: String,
    pub(crate) title: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) mime_type: Option<String>,
}

impl Listable for Resource {
    const METHOD: &'static str = method::RESOURCES_LIST;
    const ITEMS: &'static str = "resources";
}

impl Listable for ResourceTemplate {
    const METHOD: &'static str = method::RESOURCES_TEMPLATES_LIST;
    const ITEMS: &'static str = "resourceTemplates";
}

#[derive(Debug, Serialize)]
pub(crate) struct ReadResourceParams {
    pub(crate) uri: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ReadResourceResult {
    #[serde(default)]
    pub(crate) contents: Vec<ResourceContents>,
}

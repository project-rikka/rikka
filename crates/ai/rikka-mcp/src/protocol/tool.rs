use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::protocol::content::ContentBlock;
use crate::protocol::method;
use crate::protocol::page::Listable;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Tool {
    pub(crate) name: String,
    pub(crate) title: Option<String>,
    pub(crate) description: Option<String>,
    #[serde(default)]
    pub(crate) input_schema: Map<String, Value>,
    pub(crate) annotations: Option<ToolAnnotations>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct ToolAnnotations {
    pub(crate) title: Option<String>,
}

impl Listable for Tool {
    const METHOD: &'static str = method::TOOLS_LIST;
    const ITEMS: &'static str = "tools";
}

#[derive(Debug, Serialize)]
pub(crate) struct CallToolParams {
    pub(crate) name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) arguments: Option<Map<String, Value>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CallToolResult {
    #[serde(default)]
    pub(crate) content: Vec<ContentBlock>,
    pub(crate) structured_content: Option<Value>,
    pub(crate) is_error: Option<bool>,
}

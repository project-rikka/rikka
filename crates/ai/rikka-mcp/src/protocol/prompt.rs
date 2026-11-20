use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::protocol::content::ContentBlock;
use crate::protocol::method;
use crate::protocol::page::Listable;

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Prompt {
    pub(crate) name: String,
    pub(crate) title: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) arguments: Option<Vec<PromptArgument>>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct PromptArgument {
    pub(crate) name: String,
    pub(crate) title: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) required: Option<bool>,
}

impl Listable for Prompt {
    const METHOD: &'static str = method::PROMPTS_LIST;
    const ITEMS: &'static str = "prompts";
}

#[derive(Debug, Serialize)]
pub(crate) struct GetPromptParams {
    pub(crate) name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) arguments: Option<Map<String, Value>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GetPromptResult {
    pub(crate) description: Option<String>,
    #[serde(default)]
    pub(crate) messages: Vec<PromptMessage>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PromptMessage {
    pub(crate) content: ContentBlock,
}

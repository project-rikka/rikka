use serde::de::IgnoredAny;
use serde::{Deserialize, Serialize};

pub(crate) const REVISION: &str = "2025-11-25";
const KNOWN_REVISIONS: [&str; 4] = ["2024-11-05", "2025-03-26", "2025-06-18", REVISION];

const CLIENT_NAME: &str = "Rikka";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InitializeParams {
    protocol_version: &'static str,
    capabilities: ClientCapabilities,
    client_info: Implementation,
}

#[derive(Debug, Default, Serialize)]
struct ClientCapabilities {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Implementation {
    pub(crate) name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) title: Option<String>,
    #[serde(default)]
    pub(crate) version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InitializeResult {
    pub(crate) protocol_version: String,
    #[serde(default)]
    pub(crate) capabilities: ServerCapabilities,
    pub(crate) server_info: Option<Implementation>,
    pub(crate) instructions: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct ServerCapabilities {
    tools: Option<IgnoredAny>,
    prompts: Option<IgnoredAny>,
    resources: Option<IgnoredAny>,
    completions: Option<IgnoredAny>,
}

impl InitializeParams {
    pub(crate) fn new() -> InitializeParams {
        InitializeParams {
            protocol_version: REVISION,
            capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: CLIENT_NAME.to_owned(),
                title: None,
                version: env!("CARGO_PKG_VERSION").to_owned(),
                description: None,
            },
        }
    }
}

impl InitializeResult {
    pub(crate) fn speaks_known_revision(&self) -> bool {
        KNOWN_REVISIONS.contains(&self.protocol_version.as_str())
    }
}

impl ServerCapabilities {
    pub(crate) fn offers_tools(&self) -> bool {
        self.tools.is_some()
    }

    pub(crate) fn offers_prompts(&self) -> bool {
        self.prompts.is_some()
    }

    pub(crate) fn offers_resources(&self) -> bool {
        self.resources.is_some()
    }

    pub(crate) fn offers_completions(&self) -> bool {
        self.completions.is_some()
    }
}

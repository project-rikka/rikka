#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ServerTool {
    WebSearch,
    WebFetch,
}

impl ServerTool {
    pub const ALL: [ServerTool; 2] = [ServerTool::WebSearch, ServerTool::WebFetch];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            ServerTool::WebSearch => "web_search",
            ServerTool::WebFetch => "web_fetch",
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            ServerTool::WebSearch => "WebSearch",
            ServerTool::WebFetch => "WebFetch",
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<ServerTool> {
        ServerTool::ALL.into_iter().find(|tool| tool.name() == name)
    }
}

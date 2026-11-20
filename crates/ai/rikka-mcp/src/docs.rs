use std::sync::OnceLock;

use rikka_prompt::{Facts, Prompt};

pub(crate) struct Docs {
    pub(crate) tool: Prompt,
    pub(crate) resources: Prompt,
    pub(crate) servers: Prompt,
    pub(crate) facts: Facts,
}

pub(crate) fn docs() -> &'static Docs {
    static DOCS: OnceLock<Docs> = OnceLock::new();
    DOCS.get_or_init(|| {
        let tool = Prompt::read("mcp_servers", include_str!("../prompts/mcp_servers.md"))
            .expect("MCP tool prompt should parse");
        let resources = Prompt::read("mcp_resources", include_str!("../prompts/mcp_resources.md"))
            .expect("MCP resources prompt should parse");
        let servers = Prompt::read("servers", include_str!("../prompts/servers.md"))
            .expect("MCP servers prompt should parse");
        let mut facts = rikka_tools::facts().expect("shared tool prompts should parse");
        facts.set("mcp_tool", tool.name());
        facts.set("resources_tool", resources.name());
        Docs {
            tool,
            resources,
            servers,
            facts,
        }
    })
}

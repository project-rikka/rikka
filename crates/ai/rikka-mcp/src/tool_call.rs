use rikka_llm::Value;

use crate::docs::docs;

const PREFIX: &str = "mcp__";
const JOIN: &str = "__";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolCall<'a> {
    Tool {
        server: &'a str,
        tool: &'a str,
    },
    Managed {
        action: &'a str,
        server: Option<&'a str>,
    },
    Resources {
        action: &'a str,
        server: Option<&'a str>,
        uri: Option<&'a str>,
    },
}

impl<'a> ToolCall<'a> {
    #[must_use]
    pub fn parse(tool: &'a str, input: &'a Value) -> Option<ToolCall<'a>> {
        let docs = docs();
        if tool == docs.tool.name() {
            return Some(ToolCall::Managed {
                action: input.get("action").and_then(Value::as_str)?,
                server: input.get("name").and_then(Value::as_str),
            });
        }
        if tool == docs.resources.name() {
            return Some(ToolCall::Resources {
                action: input.get("action").and_then(Value::as_str)?,
                server: input.get("server").and_then(Value::as_str),
                uri: input.get("uri").and_then(Value::as_str),
            });
        }
        let (server, tool) = tool.strip_prefix(PREFIX)?.split_once(JOIN)?;
        Some(ToolCall::Tool { server, tool })
    }

    #[must_use]
    pub fn gist(&self) -> Option<String> {
        match *self {
            ToolCall::Tool { .. } => None,
            ToolCall::Managed { action, server } => Some(match server {
                Some(server) => format!("{action} {server}"),
                None => action.to_owned(),
            }),
            ToolCall::Resources {
                action,
                server,
                uri,
            } => Some(match (server, uri) {
                (Some(server), Some(uri)) => format!("{action} {server}:{uri}"),
                (Some(server), None) => format!("{action} {server}"),
                _ => action.to_owned(),
            }),
        }
    }
}

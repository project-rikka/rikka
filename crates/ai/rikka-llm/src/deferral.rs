use std::collections::BTreeSet;

use crate::content::Content;
use crate::message::Message;
use crate::request::Request;
use crate::tool_result::ToolResult;
use rikka_tool::Tool;

pub(crate) struct Deferral<'a> {
    tools: &'a [Tool],
    deferred: BTreeSet<&'a str>,
    loaded: BTreeSet<&'a str>,
    native: bool,
}

impl<'a> Deferral<'a> {
    pub(crate) fn new(request: &'a Request) -> Deferral<'a> {
        let deferred: BTreeSet<&str> = request
            .tools
            .iter()
            .filter(|tool| tool.is_deferred())
            .map(Tool::name)
            .collect();
        let loaded = request
            .messages
            .iter()
            .flat_map(Message::content)
            .filter_map(|content| match content {
                Content::ToolResult(result) => Some(result),
                _ => None,
            })
            .flat_map(ToolResult::references)
            .filter(|name| deferred.contains(name))
            .collect();
        let native = !deferred.is_empty() && request.tools.iter().any(|tool| !tool.is_deferred());

        Deferral {
            tools: &request.tools,
            deferred,
            loaded,
            native,
        }
    }

    pub(crate) fn is_native(&self) -> bool {
        self.native
    }

    pub(crate) fn references(&self, name: &str) -> bool {
        self.native && self.deferred.contains(name)
    }

    pub(crate) fn tool(&self, name: &str) -> Option<&'a Tool> {
        self.tools
            .iter()
            .find(|tool| tool.is_deferred() && tool.name() == name)
    }

    pub(crate) fn every(&self) -> Vec<&'a Tool> {
        self.tools.iter().collect()
    }

    pub(crate) fn upfront(&self) -> Vec<&'a Tool> {
        self.tools
            .iter()
            .filter(|tool| !tool.is_deferred())
            .collect()
    }

    pub(crate) fn eager(&self) -> Vec<&'a Tool> {
        self.tools
            .iter()
            .filter(|tool| !tool.is_deferred() || self.loaded.contains(tool.name()))
            .collect()
    }
}

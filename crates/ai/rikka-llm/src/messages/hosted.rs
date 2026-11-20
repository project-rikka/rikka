use std::borrow::Cow;
use std::collections::HashSet;

use serde_json::{Value, json};

use crate::content::Content;
use crate::hosted_part::{HostedOutcome, HostedPart, Source};
use crate::message::Message;
use crate::server_tool::ServerTool;
use crate::transport::text_of;

const WEB_SEARCH: &str = "web_search_20250305";
const WEB_FETCH: &str = "web_fetch_20250910";
const FETCHED_TOKENS: u32 = 50_000;

const BLOCKS: [&str; 3] = [
    "server_tool_use",
    "web_search_tool_result",
    "web_fetch_tool_result",
];

pub(super) fn is_block(value: &Value) -> bool {
    value
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| BLOCKS.contains(&kind))
}

pub(super) struct Answered<'a> {
    ids: HashSet<&'a str>,
}

impl<'a> Answered<'a> {
    pub(super) fn new(messages: &'a [Message]) -> Answered<'a> {
        let ids = messages
            .iter()
            .flat_map(Message::content)
            .filter_map(Content::as_hosted)
            .filter_map(|hosted| match hosted.part() {
                HostedPart::Result { id, .. } => Some(id.as_str()),
                HostedPart::Call { .. } => None,
            })
            .collect();
        Answered { ids }
    }

    pub(super) fn settled<'c>(&self, content: &'c [Content]) -> Cow<'c, [Content]> {
        match content.iter().any(|block| self.is_dangling(block)) {
            true => Cow::Owned(
                content
                    .iter()
                    .filter(|block| !self.is_dangling(block))
                    .cloned()
                    .collect(),
            ),
            false => Cow::Borrowed(content),
        }
    }

    fn is_dangling(&self, block: &Content) -> bool {
        block.as_hosted().is_some_and(|hosted| {
            matches!(hosted.part(), HostedPart::Call { id, .. } if !self.ids.contains(id.as_str()))
        })
    }
}

pub(super) fn declaration(tool: ServerTool) -> Value {
    match tool {
        ServerTool::WebSearch => json!({ "type": WEB_SEARCH, "name": tool.name() }),
        ServerTool::WebFetch => json!({
            "type": WEB_FETCH,
            "name": tool.name(),
            "max_content_tokens": FETCHED_TOKENS,
        }),
    }
}

pub(crate) fn part(block: &Value) -> Option<HostedPart> {
    match block.get("type").and_then(Value::as_str)? {
        "server_tool_use" => Some(HostedPart::Call {
            id: text_of(block, "id"),
            tool: ServerTool::from_name(block.get("name").and_then(Value::as_str)?)?,
            input: block.get("input").cloned().unwrap_or(Value::Null),
        }),
        "web_search_tool_result" => Some(HostedPart::Result {
            id: text_of(block, "tool_use_id"),
            tool: ServerTool::WebSearch,
            outcome: searched(&block["content"]),
        }),
        "web_fetch_tool_result" => Some(HostedPart::Result {
            id: text_of(block, "tool_use_id"),
            tool: ServerTool::WebFetch,
            outcome: fetched(&block["content"]),
        }),
        _ => None,
    }
}

fn searched(content: &Value) -> HostedOutcome {
    match content.as_array() {
        Some(results) => HostedOutcome::Found(
            results
                .iter()
                .filter(|result| result["type"] == "web_search_result")
                .map(|result| source(result, &result["title"], &result["page_age"]))
                .collect(),
        ),
        None => HostedOutcome::Failed(text_of(content, "error_code")),
    }
}

fn fetched(content: &Value) -> HostedOutcome {
    match content["type"].as_str() {
        Some("web_fetch_result") => HostedOutcome::Fetched(source(
            content,
            &content["content"]["title"],
            &content["retrieved_at"],
        )),
        _ => HostedOutcome::Failed(text_of(content, "error_code")),
    }
}

fn source(block: &Value, title: &Value, date: &Value) -> Source {
    Source::new(text_of(block, "url"), present(title), present(date))
}

fn present(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

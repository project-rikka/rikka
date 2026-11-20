use std::borrow::Cow;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Map, Value, json};

use crate::breakpoint::{self, Carries, Placement, Plan};
use crate::cache::Ttl;
use crate::content::Content;
use crate::deferral::Deferral;
use crate::document::{Document, DocumentSource};
use crate::effort::{Effort, Reveal};
use crate::error::{Error, Result};
use crate::message::Message;
use crate::messages::hosted::{self, Answered};
use crate::model::DEFAULT_MAX_TOKENS;
use crate::request::Request;
use crate::role::Role;
use crate::server_tool::ServerTool;
use crate::text::Text;
use crate::thinking_off::ThinkingOff;
use crate::tool_choice::ToolChoice;
use crate::tool_result::{Output, ToolResult};
use crate::vendor::Vendor;
use rikka_tool::Media;
use rikka_tool::Tool;
use rikka_tool::{Image, ImageSource};

const UNSTREAMED_MAX_TOKENS: u32 = 8192;
const PROVIDER: &str = "the Messages API";

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Room {
    requested: Option<u32>,
    listed: Option<u32>,
}

impl Room {
    pub(super) fn new(requested: Option<u32>, listed: Option<u32>) -> Room {
        Room { requested, listed }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Encoding<'a> {
    pub(super) room: Room,
    pub(super) stream: bool,
    pub(super) placement: Placement,
    pub(super) thinking_off: ThinkingOff,
    pub(super) server_tools: &'a [ServerTool],
}

pub(super) fn body(request: &Request, model: &str, encoding: Encoding<'_>) -> Result<Value> {
    if request.temperature.is_some() || request.top_p.is_some() {
        return Err(Error::Unsupported {
            provider: PROVIDER,
            feature: "sampling parameters",
        });
    }

    let plan = breakpoint::place(request, Carries::Everything, encoding.placement);
    let deferral = Deferral::new(request);

    let mut body = Map::new();
    body.insert("model".to_owned(), json!(model));
    body.insert(
        "max_tokens".to_owned(),
        json!(ceiling(request, encoding.room, encoding.stream)),
    );
    body.insert("messages".to_owned(), messages(request, &plan, &deferral));

    if !request.system.is_empty() {
        body.insert(
            "system".to_owned(),
            system(&request.system, &plan, request.ttl),
        );
    }
    let native = deferral.is_native();
    let declared = match native {
        true => deferral.every(),
        false => deferral.eager(),
    };
    if !declared.is_empty() || !encoding.server_tools.is_empty() {
        body.insert(
            "tools".to_owned(),
            tools(&declared, encoding.server_tools, native, &plan, request.ttl),
        );
        body.insert("tool_choice".to_owned(), tool_choice(request));
    }
    if !request.stop.is_empty() {
        body.insert("stop_sequences".to_owned(), json!(request.stop));
    }
    if let Some(thinking) = thinking(request, encoding.thinking_off) {
        body.insert("thinking".to_owned(), thinking);
    }
    if let Some(config) = output_config(request) {
        body.insert("output_config".to_owned(), config);
    }
    if plan.automatic {
        body.insert("cache_control".to_owned(), cache_control(request.ttl));
    }
    if encoding.stream {
        body.insert("stream".to_owned(), json!(true));
    }

    Ok(Value::Object(body))
}

pub(super) fn reply(content: &[Content]) -> Option<Value> {
    let request = Request::default();
    let deferral = Deferral::new(&request);
    let turn = blocks_turn(Role::Assistant, content, false, Ttl::Short, &deferral);
    (!is_empty(&turn)).then_some(turn)
}

fn ceiling(request: &Request, room: Room, stream: bool) -> u32 {
    if let Some(max_tokens) = request.max_tokens.or(room.requested) {
        return max_tokens;
    }

    let listed = room.listed.unwrap_or(DEFAULT_MAX_TOKENS);
    match stream {
        true => listed,
        false => listed.min(UNSTREAMED_MAX_TOKENS),
    }
}

fn system(blocks: &[Text], plan: &Plan, ttl: Ttl) -> Value {
    let last = blocks.len().saturating_sub(1);
    let blocks = blocks
        .iter()
        .enumerate()
        .map(|(index, text)| {
            let mark = text.is_cached() || (plan.system && index == last);
            let value = json!({ "type": "text", "text": text.as_str() });
            with_cache(value, mark, ttl)
        })
        .collect::<Vec<_>>();

    Value::Array(blocks)
}

fn tools(
    declared: &[&Tool],
    server_tools: &[ServerTool],
    native: bool,
    plan: &Plan,
    ttl: Ttl,
) -> Value {
    let last = match server_tools.is_empty() {
        true => declared.iter().rposition(|tool| !tool.is_deferred()),
        false => None,
    };
    let mut tools: Vec<Value> = declared
        .iter()
        .enumerate()
        .map(|(index, tool)| {
            let mut value = json!({
                "name": tool.name(),
                "description": tool.description(),
                "input_schema": tool.schema(),
            });
            if tool.is_strict() {
                value["strict"] = json!(true);
            }
            if tool.is_deferred() && native {
                value["defer_loading"] = json!(true);
                return value;
            }

            let mark = tool.is_cached() || (plan.tools && Some(index) == last);
            with_cache(value, mark, ttl)
        })
        .collect();
    let hosting = server_tools.len();
    tools.extend(server_tools.iter().enumerate().map(|(index, tool)| {
        with_cache(
            hosted::declaration(*tool),
            plan.tools && index + 1 == hosting,
            ttl,
        )
    }));

    Value::Array(tools)
}

fn tool_choice(request: &Request) -> Value {
    let mut choice = match &request.tool_choice {
        ToolChoice::Auto => json!({ "type": "auto" }),
        ToolChoice::Any => json!({ "type": "any" }),
        ToolChoice::Named(name) => json!({ "type": "tool", "name": name }),
        ToolChoice::None => return json!({ "type": "none" }),
    };

    if !request.parallel_tools {
        choice["disable_parallel_tool_use"] = json!(true);
    }
    choice
}

fn thinking(request: &Request, off: ThinkingOff) -> Option<Value> {
    let display = match request.reveal {
        Reveal::Hidden => "omitted",
        Reveal::Summary => "summarized",
    };

    match (request.effort, request.reveal) {
        (Some(Effort::Off), _) => Some(match off {
            ThinkingOff::Disabled => json!({ "type": "disabled" }),
            ThinkingOff::BetweenTools => json!({ "type": "between_tools" }),
        }),
        (Some(_), _) | (None, Reveal::Summary) => {
            Some(json!({ "type": "adaptive", "display": display }))
        }
        (None, Reveal::Hidden) => None,
    }
}

fn output_config(request: &Request) -> Option<Value> {
    let mut config = Map::new();

    if let Some(effort) = request.effort.and_then(Effort::name) {
        let effort = if effort == "minimal" { "low" } else { effort };
        config.insert("effort".to_owned(), json!(effort));
    }
    if let Some(format) = &request.format {
        config.insert(
            "format".to_owned(),
            json!({ "type": "json_schema", "schema": format.schema() }),
        );
    }

    (!config.is_empty()).then_some(Value::Object(config))
}

fn messages(request: &Request, plan: &Plan, deferral: &Deferral<'_>) -> Value {
    let answered = Answered::new(&request.messages);
    let messages = request
        .messages
        .iter()
        .enumerate()
        .map(|(index, message)| {
            let resumed = request
                .messages
                .get(index + 1)
                .is_none_or(|next| next.content().iter().any(is_tool_result));
            let content = match resumed {
                true => Cow::Borrowed(message.content()),
                false => answered.settled(message.content()),
            };
            turn(message, &content, plan.marks(index), request.ttl, deferral)
        })
        .filter(|turn| !is_empty(turn))
        .collect::<Vec<_>>();

    Value::Array(messages)
}

fn is_tool_result(content: &Content) -> bool {
    matches!(content, Content::ToolResult(_))
}

fn is_empty(turn: &Value) -> bool {
    match &turn["content"] {
        Value::Array(blocks) => blocks.is_empty(),
        Value::String(text) => text.is_empty(),
        _ => true,
    }
}

fn turn(
    message: &Message,
    content: &[Content],
    mark_last: bool,
    ttl: Ttl,
    deferral: &Deferral<'_>,
) -> Value {
    match message.role() {
        Role::System => json!({ "role": "system", "content": message.text() }),
        role => blocks_turn(role, content, mark_last, ttl, deferral),
    }
}

fn blocks_turn(
    role: Role,
    content: &[Content],
    mark_last: bool,
    ttl: Ttl,
    deferral: &Deferral<'_>,
) -> Value {
    let mut blocks: Vec<(Value, bool)> = Vec::with_capacity(content.len());
    let mut trailing: Vec<Value> = Vec::new();
    for content in content {
        if let Content::Text(text) = content
            && text.as_str().trim().is_empty()
        {
            spaced(&mut blocks, text.as_str());
            continue;
        }
        if let Some(value) = block(content, deferral, &mut trailing) {
            blocks.push((value, content.is_cached()));
        }
    }
    blocks.extend(trailing.into_iter().map(|value| (value, false)));

    if mark_last
        && let Some((_, mark)) = blocks.iter_mut().rev().find(|(value, _)| cacheable(value))
    {
        *mark = true;
    }

    let blocks = blocks
        .into_iter()
        .map(|(value, mark)| with_cache(value, mark, ttl))
        .collect::<Vec<_>>();

    json!({ "role": role.name(), "content": blocks })
}

fn spaced(blocks: &mut [(Value, bool)], blank: &str) {
    if let Some((last, _)) = blocks.last_mut()
        && last["type"] == "text"
        && let Some(text) = last["text"].as_str()
    {
        last["text"] = json!(format!("{text}{blank}"));
    }
}

fn cacheable(value: &Value) -> bool {
    !matches!(
        value.get("type").and_then(Value::as_str),
        Some("thinking" | "redacted_thinking")
    ) && !hosted::is_block(value)
}

fn block(content: &Content, deferral: &Deferral<'_>, trailing: &mut Vec<Value>) -> Option<Value> {
    let value = match content {
        Content::Text(text) => json!({ "type": "text", "text": text.as_str() }),
        Content::Image(image) => image_block(image),
        Content::Document(document) => document_block(document),
        Content::ToolUse(call) => json!({
            "type": "tool_use",
            "id": call.id(),
            "name": call.name(),
            "input": call.input(),
        }),
        Content::ToolResult(result) => tool_result(result, deferral, trailing),
        Content::Reasoning(reasoning) => reasoning.replay(Vendor::Anthropic)?.clone(),
        Content::Hosted(hosted) => hosted.replay(Vendor::Anthropic)?.clone(),
    };

    Some(value)
}

fn image_block(image: &Image) -> Value {
    let source = match image.source() {
        ImageSource::Inline { media, data } => json!({
            "type": "base64",
            "media_type": media.mime(),
            "data": STANDARD.encode(data),
        }),
        ImageSource::Url(url) => json!({ "type": "url", "url": url }),
    };

    json!({ "type": "image", "source": source })
}

fn document_block(document: &Document) -> Value {
    let source = match document.source() {
        DocumentSource::Inline { media, data } if *media == Media::PlainText => {
            json!({
                "type": "text",
                "media_type": media.mime(),
                "data": String::from_utf8_lossy(data),
            })
        }
        DocumentSource::Inline { media, data } => json!({
            "type": "base64",
            "media_type": media.mime(),
            "data": STANDARD.encode(data),
        }),
        DocumentSource::Url(url) => json!({ "type": "url", "url": url }),
    };

    let mut value = json!({ "type": "document", "source": source });
    if let Some(title) = document.name() {
        value["title"] = json!(title);
    }
    value
}

fn tool_result(result: &ToolResult, deferral: &Deferral<'_>, trailing: &mut Vec<Value>) -> Value {
    let mut references = Vec::new();
    let mut outputs = Vec::new();
    for output in result.output() {
        match output {
            Output::Text(text) => outputs.push(json!({ "type": "text", "text": text })),
            Output::Image(image) => outputs.push(image_block(image)),
            Output::Reference(tool) if deferral.references(tool) => {
                references.push(json!({ "type": "tool_reference", "tool_name": tool }));
            }
            Output::Reference(_) => {}
        }
    }

    let content = match references.is_empty() {
        true => outputs,
        false => {
            trailing.extend(outputs);
            references
        }
    };
    let mut value = json!({
        "type": "tool_result",
        "tool_use_id": result.id(),
        "content": content,
    });
    if result.is_failed() {
        value["is_error"] = json!(true);
    }
    value
}

fn with_cache(mut value: Value, mark: bool, ttl: Ttl) -> Value {
    if !mark || !cacheable(&value) {
        return value;
    }

    value["cache_control"] = cache_control(ttl);
    value
}

fn cache_control(ttl: Ttl) -> Value {
    match ttl {
        Ttl::Short => json!({ "type": "ephemeral" }),
        Ttl::Long => json!({ "type": "ephemeral", "ttl": "1h" }),
    }
}

use std::collections::BTreeSet;

use serde_json::{Map, Value, json};

use crate::breakpoint::{self, Plan};
use crate::cache::Ttl;
use crate::content::Content;
use crate::deferral::Deferral;
use crate::document::{Document, DocumentSource};
use crate::effort::{Effort, Reveal};
use crate::error::{Error, Result};
use crate::message::Message;
use crate::request::Request;
use crate::responses::ResponsesDialect;
use crate::role::Role;
use crate::tool_choice::ToolChoice;
use crate::tool_result::{Output, ToolResult};
use crate::vendor::Vendor;
use rikka_tool::Media;
use rikka_tool::Tool;
use rikka_tool::{Image, ImageSource};

pub(super) fn body(
    request: &Request,
    model: &str,
    stream: bool,
    dialect: ResponsesDialect,
) -> Result<Value> {
    if !request.stop.is_empty() {
        return Err(Error::Unsupported {
            provider: "the Responses API",
            feature: "stop sequences",
        });
    }

    let plan = match dialect.caches() {
        true => breakpoint::plan(request, breakpoint::Carries::Transcript),
        false => Plan::default(),
    };
    let deferral = Deferral::new(request);
    let native = dialect.defers_tools() && deferral.is_native();
    let declared = match native {
        true => deferral.upfront(),
        false => deferral.eager(),
    };
    let mut loading = native.then(|| Loading::new(&deferral));
    let input = input(request, &plan, dialect, loading.as_mut());
    let explicit = breakpoints(&input) > 0;

    let mut body = Map::new();
    body.insert("model".to_owned(), json!(model));
    body.insert("input".to_owned(), input);
    body.insert("store".to_owned(), json!(false));
    if dialect.is_hosted() {
        body.insert("include".to_owned(), json!(["reasoning.encrypted_content"]));
    }

    if !request.system.is_empty() || dialect.is_cli_dialect() {
        let instructions = request
            .system
            .iter()
            .map(|text| text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");
        body.insert("instructions".to_owned(), json!(instructions));
    }
    if !declared.is_empty() {
        body.insert("tools".to_owned(), tools(&declared));
        body.insert("tool_choice".to_owned(), tool_choice(&request.tool_choice));
    }
    if !declared.is_empty() || dialect.is_cli_dialect() {
        body.insert(
            "parallel_tool_calls".to_owned(),
            json!(request.parallel_tools),
        );
    }
    if dialect.limits() {
        if let Some(tokens) = request.max_tokens {
            body.insert("max_output_tokens".to_owned(), json!(tokens));
        }
        if let Some(temperature) = request.temperature {
            body.insert("temperature".to_owned(), json!(temperature));
        }
        if let Some(top_p) = request.top_p {
            body.insert("top_p".to_owned(), json!(top_p));
        }
    }
    if let Some(reasoning) = reasoning(request) {
        body.insert("reasoning".to_owned(), reasoning);
    }
    if let Some(format) = &request.format {
        body.insert(
            "text".to_owned(),
            json!({
                "format": {
                    "type": "json_schema",
                    "name": format.name(),
                    "schema": format.schema(),
                    "strict": format.is_strict(),
                }
            }),
        );
    }
    if let Some(key) = request.cache_key.as_ref().filter(|_| dialect.is_hosted()) {
        body.insert("prompt_cache_key".to_owned(), json!(key));
    }
    if explicit {
        body.insert(
            "prompt_cache_options".to_owned(),
            cache_options(request.ttl),
        );
    }
    if stream {
        body.insert("stream".to_owned(), json!(true));
    }

    Ok(Value::Object(body))
}

fn breakpoints(input: &Value) -> usize {
    match input {
        Value::Object(fields) => fields
            .iter()
            .map(|(key, value)| usize::from(key == "prompt_cache_breakpoint") + breakpoints(value))
            .sum(),
        Value::Array(entries) => entries.iter().map(breakpoints).sum(),
        _ => 0,
    }
}

fn cache_options(ttl: Ttl) -> Value {
    match ttl {
        Ttl::Short => json!({ "mode": "explicit" }),
        Ttl::Long => json!({ "mode": "explicit", "ttl": "30m" }),
    }
}

fn reasoning(request: &Request) -> Option<Value> {
    let mut reasoning = Map::new();

    if let Some(effort) = request.effort {
        reasoning.insert("effort".to_owned(), json!(effort.id()));
    }
    let thinking = request.effort != Some(Effort::Off);
    if thinking && request.reveal == Reveal::Summary {
        reasoning.insert("summary".to_owned(), json!("auto"));
    }

    (!reasoning.is_empty()).then_some(Value::Object(reasoning))
}

fn tools(tools: &[&Tool]) -> Value {
    Value::Array(tools.iter().map(|tool| function(tool)).collect())
}

fn function(tool: &Tool) -> Value {
    json!({
        "type": "function",
        "name": tool.name(),
        "description": tool.description(),
        "parameters": tool.schema(),
        "strict": tool.is_strict(),
    })
}

struct Loading<'a> {
    deferral: &'a Deferral<'a>,
    added: BTreeSet<&'a str>,
}

impl<'a> Loading<'a> {
    fn new(deferral: &'a Deferral<'a>) -> Loading<'a> {
        Loading {
            deferral,
            added: BTreeSet::new(),
        }
    }

    fn after(&mut self, result: &ToolResult, items: &mut Vec<Value>) {
        let added = result
            .references()
            .filter_map(|name| self.deferral.tool(name))
            .filter(|tool| self.added.insert(tool.name()))
            .map(function)
            .collect::<Vec<_>>();
        if !added.is_empty() {
            items.push(json!({
                "type": "additional_tools",
                "role": "developer",
                "tools": added,
            }));
        }
    }
}

fn tool_choice(choice: &ToolChoice) -> Value {
    match choice {
        ToolChoice::Auto => json!("auto"),
        ToolChoice::Any => json!("required"),
        ToolChoice::None => json!("none"),
        ToolChoice::Named(name) => json!({ "type": "function", "name": name }),
    }
}

fn input(
    request: &Request,
    plan: &Plan,
    dialect: ResponsesDialect,
    mut loading: Option<&mut Loading<'_>>,
) -> Value {
    let mut items = Vec::new();

    for (index, message) in request.messages.iter().enumerate() {
        let marked = plan.marks(index);
        turn(
            message,
            marked,
            dialect.is_hosted().then(|| dialect.vendor()),
            loading.as_deref_mut(),
            &mut items,
        );
    }

    Value::Array(items)
}

fn is(item: &Value, field: &str, value: &str) -> bool {
    item.get(field).and_then(Value::as_str) == Some(value)
}

fn turn(
    message: &Message,
    mark_last: bool,
    replayed: Option<Vendor>,
    loading: Option<&mut Loading<'_>>,
    items: &mut Vec<Value>,
) {
    let mut loading = loading;
    let start = items.len();
    let mut parts: Vec<Value> = Vec::new();
    let role = match message.role() {
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::System => "developer",
    };

    for content in message.content() {
        match content {
            Content::Text(text) => parts.push(match message.role() {
                Role::Assistant => spoken_part(text.as_str(), text.is_cached()),
                Role::User | Role::System => text_part(text.as_str(), text.is_cached()),
            }),
            Content::Image(image) => parts.push(image_part(image)),
            Content::Document(document) => parts.push(document_part(document)),
            Content::ToolUse(call) => {
                flush(role, &mut parts, items);
                items.push(json!({
                    "type": "function_call",
                    "call_id": call.id(),
                    "name": call.name(),
                    "arguments": call.input().to_string(),
                }));
            }
            Content::ToolResult(result) => {
                flush(role, &mut parts, items);
                items.push(tool_output(result));
                if let Some(loading) = loading.as_deref_mut() {
                    loading.after(result, items);
                }
            }
            Content::Reasoning(reasoning) => {
                if let Some(payload) = replayed.and_then(|vendor| reasoning.replay(vendor)) {
                    flush(role, &mut parts, items);
                    items.push(payload.clone());
                }
            }
            Content::Hosted(_) => {}
        }
    }

    flush(role, &mut parts, items);

    if mark_last {
        mark(&mut items[start..]);
    }
}

fn flush(role: &str, parts: &mut Vec<Value>, items: &mut Vec<Value>) {
    if parts.is_empty() {
        return;
    }
    items.push(json!({
        "type": "message",
        "role": role,
        "content": std::mem::take(parts),
    }));
}

fn mark(items: &mut [Value]) {
    for item in items.iter_mut().rev() {
        for field in ["content", "output"] {
            if let Some(part) = item
                .get_mut(field)
                .and_then(Value::as_array_mut)
                .and_then(|parts| parts.last_mut())
            {
                part["prompt_cache_breakpoint"] = breakpoint();
                return;
            }
        }
    }
}

fn breakpoint() -> Value {
    json!({ "mode": "explicit" })
}

fn text_part(text: &str, cached: bool) -> Value {
    marked(json!({ "type": "input_text", "text": text }), cached)
}

fn spoken_part(text: &str, cached: bool) -> Value {
    marked(json!({ "type": "output_text", "text": text }), cached)
}

fn image_part(image: &Image) -> Value {
    let url = match image.source() {
        ImageSource::Inline { media, data } => media.data_url(data),
        ImageSource::Url(url) => url.clone(),
    };

    marked(
        json!({ "type": "input_image", "image_url": url, "detail": "auto" }),
        image.is_cached(),
    )
}

fn document_part(document: &Document) -> Value {
    let name = document.name().unwrap_or("document");

    match document.source() {
        DocumentSource::Inline { media, data } if *media == Media::PlainText => {
            text_part(&String::from_utf8_lossy(data), document.is_cached())
        }
        DocumentSource::Inline { media, data } => marked(
            json!({
                "type": "input_file",
                "filename": format!("{name}.{}", media.extension()),
                "file_data": media.data_url(data),
            }),
            document.is_cached(),
        ),
        DocumentSource::Url(url) => marked(
            json!({ "type": "input_file", "file_url": url }),
            document.is_cached(),
        ),
    }
}

fn marked(mut part: Value, cached: bool) -> Value {
    if cached {
        part["prompt_cache_breakpoint"] = breakpoint();
    }
    part
}

fn tool_output(result: &ToolResult) -> Value {
    let mut output = result
        .output()
        .iter()
        .filter_map(|output| match output {
            Output::Text(text) => Some(text_part(text, false)),
            Output::Image(image) => Some(image_part(image)),
            Output::Reference(_) => None,
        })
        .collect::<Vec<_>>();
    if result.is_cached()
        && let Some(last) = output.last_mut()
        && is(last, "type", "input_text")
    {
        last["prompt_cache_breakpoint"] = breakpoint();
    }

    json!({
        "type": "function_call_output",
        "call_id": result.id(),
        "output": output,
    })
}

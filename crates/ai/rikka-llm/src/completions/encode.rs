use serde_json::{Map, Value, json};

use crate::completions::CompletionsDialect;
use crate::content::Content;
use crate::deferral::Deferral;
use crate::document::{Document, DocumentSource};
use crate::message::Message;
use crate::request::Request;
use crate::role::Role;
use crate::tool_choice::ToolChoice;
use crate::tool_result::{Output, ToolResult};
use rikka_tool::Media;
use rikka_tool::Tool;
use rikka_tool::{Image, ImageSource};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Ceiling {
    Completion,
    Legacy,
}

pub(super) fn body(
    request: &Request,
    model: &str,
    ceiling: Ceiling,
    dialect: CompletionsDialect,
    stream: bool,
) -> Value {
    let mut body = Map::new();
    body.insert("model".to_owned(), json!(model));
    body.insert("messages".to_owned(), messages(request, dialect));

    let declared = Deferral::new(request).eager();
    if !declared.is_empty() {
        body.insert("tools".to_owned(), tools(&declared, dialect));
        body.insert("tool_choice".to_owned(), tool_choice(&request.tool_choice));
        body.insert(
            "parallel_tool_calls".to_owned(),
            json!(request.parallel_tools),
        );
    }
    if let Some(tokens) = request.max_tokens {
        let field = match ceiling {
            Ceiling::Completion => "max_completion_tokens",
            Ceiling::Legacy => "max_tokens",
        };
        body.insert(field.to_owned(), json!(tokens));
    }
    if let Some(temperature) = request.temperature {
        body.insert("temperature".to_owned(), json!(temperature));
    }
    if let Some(top_p) = request.top_p {
        body.insert("top_p".to_owned(), json!(top_p));
    }
    if !request.stop.is_empty() {
        body.insert("stop".to_owned(), json!(request.stop));
    }
    if let Some(format) = &request.format {
        body.insert(
            "response_format".to_owned(),
            json!({
                "type": "json_schema",
                "json_schema": {
                    "name": format.name(),
                    "schema": format.schema(),
                    "strict": format.is_strict(),
                }
            }),
        );
    }
    if stream {
        body.insert("stream".to_owned(), json!(true));
        body.insert(
            "stream_options".to_owned(),
            json!({ "include_usage": true }),
        );
    }

    if let Some(effort) = request.effort {
        match dialect {
            CompletionsDialect::Compatible => {
                body.insert("reasoning_effort".to_owned(), json!(effort.id()))
            }
            CompletionsDialect::OpenRouter => {
                body.insert("reasoning".to_owned(), json!({ "effort": effort.id() }))
            }
        };
    }

    Value::Object(body)
}

fn tools(tools: &[&Tool], dialect: CompletionsDialect) -> Value {
    let tools = tools
        .iter()
        .map(|tool| {
            let mut function = json!({
                "name": tool.name(),
                "description": tool.description(),
                "parameters": tool.schema(),
            });
            if tool.is_strict() || dialect.declares_leniency() {
                function["strict"] = json!(tool.is_strict());
            }
            json!({ "type": "function", "function": function })
        })
        .collect::<Vec<_>>();

    Value::Array(tools)
}

fn tool_choice(choice: &ToolChoice) -> Value {
    match choice {
        ToolChoice::Auto => json!("auto"),
        ToolChoice::Any => json!("required"),
        ToolChoice::None => json!("none"),
        ToolChoice::Named(name) => json!({
            "type": "function",
            "function": { "name": name },
        }),
    }
}

fn messages(request: &Request, dialect: CompletionsDialect) -> Value {
    let mut messages = Vec::new();

    for text in &request.system {
        messages.push(json!({ "role": "system", "content": text.as_str() }));
    }
    for message in &request.messages {
        turn(message, dialect, &mut messages);
    }

    Value::Array(messages)
}

fn turn(message: &Message, dialect: CompletionsDialect, messages: &mut Vec<Value>) {
    let mut parts = Vec::new();
    let mut calls = Vec::new();
    let mut results = Vec::new();
    let mut pictures = Vec::new();
    let mut details = Vec::new();

    for content in message.content() {
        match content {
            Content::Text(text) => parts.push(json!({ "type": "text", "text": text.as_str() })),
            Content::Image(image) => parts.push(image_part(image)),
            Content::Document(document) => parts.push(document_part(document)),
            Content::ToolUse(call) => calls.push(json!({
                "id": call.id(),
                "type": "function",
                "function": {
                    "name": call.name(),
                    "arguments": call.input().to_string(),
                },
            })),
            Content::ToolResult(result) => {
                results.push(tool_message(result));
                pictures.extend(images_of(result));
            }
            Content::Reasoning(reasoning) => {
                if dialect.replays()
                    && let Some(payload) = reasoning.replay(dialect.vendor())
                    && let Some(carried) = payload.as_array()
                {
                    details.extend(carried.iter().cloned());
                }
            }
            Content::Hosted(_) => {}
        }
    }

    let role = match message.role() {
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::System => "system",
    };

    messages.append(&mut results);

    if !pictures.is_empty() {
        messages.push(json!({ "role": "user", "content": pictures }));
    }

    if !parts.is_empty() || !calls.is_empty() || !details.is_empty() {
        let mut value = json!({ "role": role, "content": content(parts) });
        if !calls.is_empty() {
            value["tool_calls"] = Value::Array(calls);
        }
        if !details.is_empty() {
            value["reasoning_details"] = Value::Array(details);
        }
        messages.push(value);
    }
}

fn content(parts: Vec<Value>) -> Value {
    let single_text =
        parts.len() == 1 && parts[0].get("type").and_then(Value::as_str) == Some("text");

    match (parts.is_empty(), single_text) {
        (true, _) => Value::Null,
        (false, true) => parts[0]["text"].clone(),
        (false, false) => Value::Array(parts),
    }
}

fn image_part(image: &Image) -> Value {
    let url = match image.source() {
        ImageSource::Inline { media, data } => media.data_url(data),
        ImageSource::Url(url) => url.clone(),
    };

    json!({ "type": "image_url", "image_url": { "url": url } })
}

fn document_part(document: &Document) -> Value {
    let name = document.name().unwrap_or("document");

    match document.source() {
        DocumentSource::Inline { media, data } if *media == Media::PlainText => {
            json!({ "type": "text", "text": String::from_utf8_lossy(data) })
        }
        DocumentSource::Inline { media, data } => json!({
            "type": "file",
            "file": {
                "filename": format!("{name}.{}", media.extension()),
                "file_data": media.data_url(data),
            },
        }),
        DocumentSource::Url(url) => json!({ "type": "text", "text": url }),
    }
}

fn tool_message(result: &ToolResult) -> Value {
    json!({
        "role": "tool",
        "tool_call_id": result.id(),
        "content": result.text(),
    })
}

fn images_of(result: &ToolResult) -> Vec<Value> {
    result
        .output()
        .iter()
        .filter_map(|output| match output {
            Output::Image(image) => Some(image_part(image)),
            Output::Text(_) | Output::Reference(_) => None,
        })
        .collect()
}

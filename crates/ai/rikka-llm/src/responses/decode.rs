use serde_json::{Value, json};

use crate::content::Content;
use crate::error::{Error, Result};
use crate::reasoning::Reasoning;
use crate::response::Response;
use crate::stop::Stop;
use crate::text::Text;
use crate::tool_use::ToolUse;
use crate::transport::{arguments, count_of, text_of, unless_cut_short};
use crate::vendor::Vendor;
use rikka_tool::Usage;

pub(crate) fn response(body: &Value, vendor: Vendor) -> Result<Response> {
    if text_of(body, "status") == "failed" {
        let message = body
            .get("error")
            .map(|error| text_of(error, "message"))
            .unwrap_or_default();
        return Err(Error::Rejected(message));
    }

    let mut content = Vec::new();
    let mut declined = None;
    let mut unreadable = None;

    for item in parts(body, "output") {
        match text_of(item, "type").as_str() {
            "function_call" => match call(item) {
                Ok(call) => content.push(Content::ToolUse(call)),
                Err(error) => {
                    unreadable.get_or_insert(error);
                }
            },
            _ => collect(item, vendor, &mut content, &mut declined),
        }
    }

    let stop = stop(body, declined);
    if let Some(error) = unreadable {
        unless_cut_short(Err::<(), Error>(error), &stop)?;
    }

    Ok(Response::new(
        text_of(body, "id"),
        text_of(body, "model"),
        content,
        stop,
        usage(body.get("usage")),
    ))
}

fn collect(
    item: &Value,
    vendor: Vendor,
    content: &mut Vec<Content>,
    declined: &mut Option<String>,
) {
    match text_of(item, "type").as_str() {
        "message" => {
            for part in parts(item, "content") {
                match text_of(part, "type").as_str() {
                    "output_text" => content.push(Content::Text(Text::new(text_of(part, "text")))),
                    "refusal" => *declined = Some(text_of(part, "refusal")),
                    _ => {}
                }
            }
        }
        "reasoning" => {
            let summary = parts(item, "summary")
                .iter()
                .map(|part| text_of(part, "text"))
                .collect::<Vec<_>>()
                .join("\n");

            content.push(Content::Reasoning(Reasoning::opaque(
                vendor,
                summary,
                replayable(item),
            )));
        }
        _ => {}
    }
}

fn replayable(item: &Value) -> Value {
    let mut payload = json!({
        "type": "reasoning",
        "id": text_of(item, "id"),
        "summary": item.get("summary").cloned().unwrap_or_else(|| json!([])),
    });

    if let Some(encrypted) = item
        .get("encrypted_content")
        .filter(|value| !value.is_null())
    {
        payload["encrypted_content"] = encrypted.clone();
    }
    payload
}

pub(crate) fn call(item: &Value) -> Result<ToolUse> {
    Ok(ToolUse::new(
        text_of(item, "call_id"),
        text_of(item, "name"),
        arguments(&text_of(item, "arguments"))?,
    ))
}

fn parts<'a>(item: &'a Value, field: &str) -> &'a [Value] {
    item.get(field)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}

fn stop(body: &Value, declined: Option<String>) -> Stop {
    if let Some(reason) = declined {
        return Stop::Declined(reason);
    }

    let incomplete = body
        .get("incomplete_details")
        .filter(|details| !details.is_null())
        .map(|details| text_of(details, "reason"));

    match incomplete.as_deref() {
        Some("max_output_tokens") => return Stop::MaxTokens,
        Some("content_filter") => return Stop::Filtered,
        _ => {}
    }

    let wants_tools = body
        .get("output")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items
                .iter()
                .any(|item| text_of(item, "type") == "function_call")
        });

    match wants_tools {
        true => Stop::ToolUse,
        false => Stop::End,
    }
}

pub(crate) fn usage(value: Option<&Value>) -> Usage {
    let Some(value) = value else {
        return Usage::default();
    };

    let prompt = value.get("input_tokens_details");

    Usage {
        reasoning: value
            .get("output_tokens_details")
            .map(|details| count_of(details, "reasoning_tokens"))
            .unwrap_or_default(),
        ..Usage::of_prompt(
            count_of(value, "input_tokens"),
            count_of(value, "output_tokens"),
            prompt
                .map(|it| count_of(it, "cached_tokens"))
                .unwrap_or_default(),
            prompt
                .map(|it| count_of(it, "cache_write_tokens"))
                .unwrap_or_default(),
        )
    }
}

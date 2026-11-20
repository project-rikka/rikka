use serde_json::Value;

use crate::content::Content;
use crate::error::{Error, Result};
use crate::hosted::Hosted;
use crate::reasoning::Reasoning;
use crate::response::Response;
use crate::stop::Stop;
use crate::text::Text;
use crate::tool_use::ToolUse;
use crate::transport::{count_of, text_of};
use crate::vendor::Vendor;
use rikka_tool::Usage;

pub(crate) fn response(body: &Value) -> Result<Response> {
    let content = body
        .get("content")
        .and_then(Value::as_array)
        .map(|blocks| blocks.iter().filter_map(block).collect())
        .unwrap_or_default();

    Ok(Response::new(
        text_of(body, "id"),
        text_of(body, "model"),
        content,
        stop(body),
        usage(body.get("usage")),
    ))
}

pub(crate) fn block(value: &Value) -> Option<Content> {
    match value.get("type").and_then(Value::as_str)? {
        "text" => Some(Content::Text(Text::new(text_of(value, "text")))),
        "thinking" => Some(Content::Reasoning(Reasoning::opaque(
            Vendor::Anthropic,
            text_of(value, "thinking"),
            value.clone(),
        ))),
        "redacted_thinking" => Some(Content::Reasoning(Reasoning::opaque(
            Vendor::Anthropic,
            String::new(),
            value.clone(),
        ))),
        "tool_use" => Some(Content::ToolUse(ToolUse::new(
            text_of(value, "id"),
            text_of(value, "name"),
            value.get("input").cloned().unwrap_or(Value::Null),
        ))),
        _ => Hosted::read(Vendor::Anthropic, value.clone()).map(Content::Hosted),
    }
}

pub(crate) fn stop(body: &Value) -> Stop {
    let reason = body.get("stop_reason").and_then(Value::as_str);

    match reason {
        Some("tool_use") => Stop::ToolUse,
        Some("max_tokens" | "model_context_window_exceeded") => Stop::MaxTokens,
        Some("stop_sequence") => Stop::Sequence(text_of(body, "stop_sequence")),
        Some("refusal") => Stop::Declined(declined(body)),
        Some("pause_turn") => Stop::Paused,
        _ => Stop::End,
    }
}

fn declined(body: &Value) -> String {
    let Some(details) = body.get("stop_details") else {
        return "no reason given".to_owned();
    };

    let explanation = text_of(details, "explanation");
    match explanation.is_empty() {
        false => explanation,
        true => match text_of(details, "category").is_empty() {
            false => text_of(details, "category"),
            true => "no reason given".to_owned(),
        },
    }
}

pub(crate) fn usage(value: Option<&Value>) -> Usage {
    let Some(value) = value else {
        return Usage::default();
    };

    Usage {
        input: count_of(value, "input_tokens"),
        output: count_of(value, "output_tokens"),
        cache_read: count_of(value, "cache_read_input_tokens"),
        cache_write: count_of(value, "cache_creation_input_tokens"),
        reasoning: value
            .get("output_tokens_details")
            .map(|details| count_of(details, "thinking_tokens"))
            .unwrap_or_default(),
    }
}

pub(crate) fn stream_error(value: &Value) -> Error {
    let Some(error) = value.get("error") else {
        return Error::Malformed("the stream carried an error with no body".to_owned());
    };

    match text_of(error, "type").as_str() {
        "overloaded_error" | "api_error" => Error::Overloaded,
        "rate_limit_error" => Error::RateLimited { retry_after: None },
        "authentication_error" | "permission_error" => Error::Unauthorized,
        _ => Error::Rejected(text_of(error, "message")),
    }
}

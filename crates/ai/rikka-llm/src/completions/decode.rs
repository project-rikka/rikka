use serde_json::Value;

use crate::completions::CompletionsDialect;
use crate::completions::thinking::Thinking;
use crate::content::Content;
use crate::error::Result;
use crate::random;
use crate::reasoning::Reasoning;
use crate::response::Response;
use crate::stop::Stop;
use crate::text::Text;
use crate::tool_use::ToolUse;
use crate::transport::{arguments, count_of, text_of, unless_cut_short};
use rikka_tool::Usage;

const CALL_ID_BYTES: usize = 12;

pub(crate) fn response(body: &Value, dialect: CompletionsDialect) -> Result<Response> {
    let choice = body
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first());

    let message = choice.and_then(|choice| choice.get("message"));
    let mut stop = stop(choice, message.and_then(refusal));

    let mut content = Vec::new();
    if let Some(message) = message {
        collect(message, dialect, &stop, &mut content)?;
    }
    if stop == Stop::End
        && content
            .iter()
            .any(|part| matches!(part, Content::ToolUse(_)))
    {
        stop = Stop::ToolUse;
    }

    Ok(Response::new(
        text_of(body, "id"),
        text_of(body, "model"),
        content,
        stop,
        usage(body.get("usage")),
    ))
}

pub(crate) fn reasoning_of(value: &Value) -> Option<String> {
    for field in ["reasoning_content", "reasoning"] {
        let text = value.get(field).and_then(Value::as_str).unwrap_or_default();
        if !text.is_empty() {
            return Some(text.to_owned());
        }
    }
    None
}

pub(crate) fn arguments_of(function: &Value) -> String {
    match function.get("arguments") {
        Some(Value::String(text)) => text.clone(),
        None | Some(Value::Null) => String::new(),
        Some(other) => other.to_string(),
    }
}

pub(crate) fn minted_call_id() -> String {
    format!("call_{}", random::hex(&random::bytes::<CALL_ID_BYTES>()))
}

fn collect(
    message: &Value,
    dialect: CompletionsDialect,
    stop: &Stop,
    content: &mut Vec<Content>,
) -> Result<()> {
    let mut text = text_of(message, "content");
    let mut inline = String::new();
    if dialect.thinks_inline() {
        let mut thinking = Thinking::default();
        let mut split = thinking.split(&text);
        let rest = thinking.finish();
        split.reasoning.push_str(&rest.reasoning);
        split.text.push_str(&rest.text);
        inline = split.reasoning;
        text = split.text;
    }

    if let Some(reasoning) = reasoning(message, dialect, inline) {
        content.push(Content::Reasoning(reasoning));
    }
    if !text.is_empty() {
        content.push(Content::Text(Text::new(text)));
    }

    for call in message
        .get("tool_calls")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
    {
        content.extend(unless_cut_short(tool_call(call), stop)?.map(Content::ToolUse));
    }

    Ok(())
}

fn tool_call(call: &Value) -> Result<ToolUse> {
    let function = call.get("function").unwrap_or(&Value::Null);

    let id = match text_of(call, "id") {
        id if id.is_empty() => minted_call_id(),
        id => id,
    };
    Ok(ToolUse::new(
        id,
        text_of(function, "name"),
        arguments(&arguments_of(function))?,
    ))
}

fn reasoning(message: &Value, dialect: CompletionsDialect, inline: String) -> Option<Reasoning> {
    let text = reasoning_of(message).unwrap_or(inline);
    let details = dialect
        .replays()
        .then(|| message.get("reasoning_details"))
        .flatten()
        .filter(|details| details.as_array().is_some_and(|list| !list.is_empty()));

    match details {
        Some(details) => Some(Reasoning::opaque(dialect.vendor(), text, details.clone())),
        None if !text.is_empty() => Some(Reasoning::new(text)),
        None => None,
    }
}

fn refusal(message: &Value) -> Option<String> {
    let refusal = message.get("refusal")?.as_str()?;
    (!refusal.is_empty()).then(|| refusal.to_owned())
}

pub(crate) fn stop(choice: Option<&Value>, declined: Option<String>) -> Stop {
    if let Some(reason) = declined {
        return Stop::Declined(reason);
    }

    let reason = choice
        .map(|choice| text_of(choice, "finish_reason"))
        .unwrap_or_default();

    match reason.as_str() {
        "tool_calls" | "function_call" => Stop::ToolUse,
        "length" => Stop::MaxTokens,
        "content_filter" => Stop::Filtered,
        _ => Stop::End,
    }
}

pub(crate) fn usage(value: Option<&Value>) -> Usage {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Usage::default();
    };

    let prompt = value.get("prompt_tokens_details");

    Usage {
        reasoning: value
            .get("completion_tokens_details")
            .map(|details| count_of(details, "reasoning_tokens"))
            .unwrap_or_default(),
        ..Usage::of_prompt(
            count_of(value, "prompt_tokens"),
            count_of(value, "completion_tokens"),
            prompt
                .map(|it| count_of(it, "cached_tokens"))
                .unwrap_or_default(),
            prompt
                .map(|it| count_of(it, "cache_write_tokens"))
                .unwrap_or_default(),
        )
    }
}

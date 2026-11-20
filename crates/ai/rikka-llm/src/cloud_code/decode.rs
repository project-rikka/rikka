use serde_json::{Value, json};

use crate::content::Content;
use crate::error::{Error, Result};
use crate::reasoning::Reasoning;
use crate::response::Response;
use crate::stop::Stop;
use crate::text::Text;
use crate::tool_use::ToolUse;
use crate::transport::{count_of, text_of};
use crate::vendor::Vendor;
use rikka_tool::Usage;

pub(crate) fn response(body: &Value) -> Result<Response> {
    let answer = answer(body);
    let candidate = candidate(answer);

    if let Some(blocked) = blocked(answer) {
        return Err(Error::Declined(blocked));
    }
    let Some(candidate) = candidate else {
        return Err(Error::Malformed(
            "the answer carried no candidate".to_owned(),
        ));
    };

    let mut content = Vec::new();
    collect(parts(candidate), &mut content);

    let stop = stop(text_of(candidate, "finishReason").as_str(), &content);
    Ok(Response::new(
        text_of(answer, "responseId"),
        text_of(answer, "modelVersion"),
        content,
        stop,
        usage(answer.get("usageMetadata")),
    ))
}

pub(crate) fn answer(body: &Value) -> &Value {
    body.get("response").unwrap_or(body)
}

pub(crate) fn candidate(answer: &Value) -> Option<&Value> {
    answer
        .get("candidates")
        .and_then(Value::as_array)
        .and_then(|candidates| candidates.first())
}

pub(crate) fn parts(candidate: &Value) -> &[Value] {
    candidate
        .get("content")
        .and_then(|content| content.get("parts"))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}

pub(crate) fn collect(parts: &[Value], content: &mut Vec<Content>) {
    let mut ordinal = 0;

    for part in parts {
        let thought = part.get("thought").and_then(Value::as_bool) == Some(true);

        if thought {
            content.push(Content::Reasoning(Reasoning::opaque(
                Vendor::Antigravity,
                text_of(part, "text"),
                part.clone(),
            )));
            continue;
        }
        if let Some(call) = tool_use(part, ordinal) {
            ordinal += 1;
            content.push(Content::ToolUse(call));
            continue;
        }
        if let Some(signature) = signature(part) {
            content.push(Content::Reasoning(Reasoning::opaque(
                Vendor::Antigravity,
                "",
                json!({ "text": "", "thoughtSignature": signature }),
            )));
        }

        if let Some(text) = part.get("text").and_then(Value::as_str)
            && !text.is_empty()
        {
            content.push(Content::Text(Text::new(text)));
        }
    }
}

pub(crate) fn tool_use(part: &Value, ordinal: usize) -> Option<ToolUse> {
    let call = part.get("functionCall")?;
    let name = text_of(call, "name");
    let issued = text_of(call, "id");
    let id = match issued.is_empty() {
        true => format!("{name}-{ordinal}"),
        false => issued,
    };

    let call_part = ToolUse::new(
        id,
        name,
        call.get("args").cloned().unwrap_or_else(|| json!({})),
    );

    Some(match signature(part) {
        Some(signature) => call_part.signed(Vendor::Antigravity, signature),
        None => call_part,
    })
}

pub(crate) fn signature(part: &Value) -> Option<&str> {
    part.get("thoughtSignature")
        .and_then(Value::as_str)
        .filter(|signature| !signature.is_empty())
}

pub(crate) fn stop(reason: &str, content: &[Content]) -> Stop {
    let wants_tools = content
        .iter()
        .any(|block| matches!(block, Content::ToolUse(_)));

    match reason {
        "MAX_TOKENS" => Stop::MaxTokens,
        "SAFETY" | "RECITATION" | "BLOCKLIST" | "PROHIBITED_CONTENT" | "SPII" => Stop::Filtered,
        "MALFORMED_FUNCTION_CALL" => {
            Stop::Declined("the model asked for a tool it could not spell".to_owned())
        }
        _ if wants_tools => Stop::ToolUse,
        _ => Stop::End,
    }
}

fn blocked(answer: &Value) -> Option<String> {
    let feedback = answer.get("promptFeedback")?;
    match text_of(feedback, "blockReason") {
        reason if reason.is_empty() => None,
        reason => Some(reason),
    }
}

pub(crate) fn usage(value: Option<&Value>) -> Usage {
    let Some(value) = value else {
        return Usage::default();
    };

    let reasoning = count_of(value, "thoughtsTokenCount");

    Usage {
        reasoning,
        ..Usage::of_prompt(
            count_of(value, "promptTokenCount"),
            count_of(value, "candidatesTokenCount") + reasoning,
            count_of(value, "cachedContentTokenCount"),
            0,
        )
    }
}

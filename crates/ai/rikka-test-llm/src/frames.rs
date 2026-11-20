use rikka_llm::{Content, Stop, Usage};
use serde_json::{Value, json};

pub(crate) fn json(status: u16, body: &Value) -> Vec<u8> {
    http(status, "application/json", body.to_string().as_bytes())
}

pub(crate) fn text(status: u16, body: &str) -> Vec<u8> {
    http(status, "text/plain", body.as_bytes())
}

pub(crate) fn events(frames: impl IntoIterator<Item = String>) -> Vec<u8> {
    let mut body = String::new();
    for frame in frames {
        body.push_str("data: ");
        body.push_str(&frame);
        body.push_str("\n\n");
    }
    http(200, "text/event-stream", body.as_bytes())
}

pub(crate) fn message(id: &str, blocks: Vec<Value>, stop: &Stop, usage: Usage) -> Value {
    let mut response = json!({
        "id": id,
        "type": "message",
        "role": "assistant",
        "model": "scripted",
        "content": blocks,
        "usage": {
            "input_tokens": usage.input,
            "output_tokens": usage.output,
            "cache_read_input_tokens": usage.cache_read,
            "cache_creation_input_tokens": usage.cache_write,
        },
    });
    let reason = match stop {
        Stop::End => "end_turn",
        Stop::ToolUse => "tool_use",
        Stop::MaxTokens => "max_tokens",
        Stop::Paused => "pause_turn",
        Stop::Sequence(sequence) => {
            response["stop_sequence"] = json!(sequence);
            "stop_sequence"
        }
        Stop::Declined(reason) => {
            response["stop_details"] = json!({ "explanation": reason });
            "refusal"
        }
        other => panic!("a stop of {other:?} cannot be scripted over the wire"),
    };
    response["stop_reason"] = json!(reason);
    response
}

pub(crate) fn streamed(message: &Value) -> Vec<String> {
    let mut frames = vec![json!({
        "type": "message_start",
        "message": {
            "id": message["id"],
            "model": message["model"],
            "usage": {
                "input_tokens": message["usage"]["input_tokens"],
                "cache_read_input_tokens": message["usage"]["cache_read_input_tokens"],
                "cache_creation_input_tokens": message["usage"]["cache_creation_input_tokens"],
            },
        },
    })];
    let blocks = message["content"].as_array().map_or(&[][..], Vec::as_slice);
    for (index, block) in blocks.iter().enumerate() {
        let (start, deltas) = match block["type"].as_str() {
            Some("thinking") => (
                json!({ "type": "thinking", "thinking": "" }),
                vec![
                    json!({ "type": "thinking_delta", "thinking": block["thinking"] }),
                    json!({ "type": "signature_delta", "signature": block["signature"] }),
                ],
            ),
            Some("tool_use" | "server_tool_use") => {
                let mut start = block.clone();
                start["input"] = json!({});
                (
                    start,
                    vec![
                        json!({ "type": "input_json_delta", "partial_json": block["input"].to_string() }),
                    ],
                )
            }
            Some("text") => (
                json!({ "type": "text", "text": "" }),
                vec![json!({ "type": "text_delta", "text": block["text"] })],
            ),
            _ => (block.clone(), Vec::new()),
        };
        frames
            .push(json!({ "type": "content_block_start", "index": index, "content_block": start }));
        for delta in deltas {
            frames.push(json!({ "type": "content_block_delta", "index": index, "delta": delta }));
        }
        frames.push(json!({ "type": "content_block_stop", "index": index }));
    }
    let mut ended = json!({ "stop_reason": message["stop_reason"] });
    for carried in ["stop_sequence", "stop_details"] {
        if let Some(value) = message.get(carried) {
            ended[carried] = value.clone();
        }
    }
    frames.push(json!({
        "type": "message_delta",
        "delta": ended,
        "usage": { "output_tokens": message["usage"]["output_tokens"] },
    }));
    frames.push(json!({ "type": "message_stop" }));
    frames.into_iter().map(|frame| frame.to_string()).collect()
}

pub(crate) fn block(content: &Content) -> Value {
    match content {
        Content::Text(text) => json!({ "type": "text", "text": text.as_str() }),
        Content::Reasoning(reasoning) => json!({
            "type": "thinking",
            "thinking": reasoning.text(),
            "signature": "scripted",
        }),
        Content::ToolUse(call) => json!({
            "type": "tool_use",
            "id": call.id(),
            "name": call.name(),
            "input": call.input(),
        }),
        other => panic!("a scripted answer holds text, thoughts and calls, not {other:?}"),
    }
}

fn http(status: u16, kind: &str, body: &[u8]) -> Vec<u8> {
    let mut reply = format!(
        "HTTP/1.1 {status} Reply\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    reply.extend_from_slice(body);
    reply
}

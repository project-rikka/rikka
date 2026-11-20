use serde_json::Value;

use crate::effort::Effort;
use crate::error::{Error, Result};
use crate::listing::{ListedModel, day_of_rfc3339, efforts, output_tokens, text, tokens};
use crate::server_tool::ServerTool;
use crate::thinking_off::ThinkingOff;
use crate::transport::Transport;

const PAGE: &str = "1000";

pub async fn models(transport: &Transport) -> Result<Vec<ListedModel>> {
    let mut listed = Vec::new();
    let mut after: Option<String> = None;
    loop {
        let mut page = transport.endpoint().clone();
        page.query_pairs_mut().append_pair("limit", PAGE);
        if let Some(after) = &after {
            page.query_pairs_mut().append_pair("after_id", after);
        }
        let answer = transport.with_endpoint(page).get().await?;
        let models = answer
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| Error::Malformed("the model listing holds no data".to_owned()))?;
        listed.extend(models.iter().filter_map(model));

        let more = answer.get("has_more").and_then(Value::as_bool) == Some(true);
        after = text(&answer, "last_id");
        if !more || after.is_none() {
            return Ok(listed);
        }
    }
}

fn model(model: &Value) -> Option<ListedModel> {
    let id = text(model, "id")?;
    let lifecycle = model.get("lifecycle").and_then(Value::as_str);
    if lifecycle == Some("retired") {
        return None;
    }
    let capabilities = model.get("capabilities").filter(|found| found.is_object());
    let thinking_off = capabilities.and_then(off);

    Some(
        ListedModel::new(id)
            .with_name(text(model, "display_name"))
            .with_context(tokens(model, "max_input_tokens"))
            .with_output(output_tokens(model, "max_tokens"))
            .with_vision(capabilities.map(|found| supported(&found["image_input"])))
            .with_efforts(capabilities.map(|found| levels(found, thinking_off)))
            .with_thinking_off(thinking_off)
            .with_server_tools(capabilities.map(server_tools))
            .with_released(
                model
                    .get("created_at")
                    .and_then(Value::as_str)
                    .and_then(day_of_rfc3339),
            )
            .with_deprecated(lifecycle == Some("deprecated")),
    )
}

fn levels(capabilities: &Value, thinking_off: Option<ThinkingOff>) -> Vec<Effort> {
    let mut levels = capabilities["effort"]
        .as_object()
        .map(|effort| {
            efforts(
                effort
                    .iter()
                    .filter(|(_, level)| supported(level))
                    .map(|(name, _)| name.as_str()),
            )
        })
        .unwrap_or_default();
    if thinking_off.is_some() {
        levels.push(Effort::Off);
    }
    levels
}

fn server_tools(capabilities: &Value) -> Vec<ServerTool> {
    let mut tools = Vec::with_capacity(ServerTool::ALL.len());
    if supported(&capabilities["server_tools"]["web_search"]) {
        tools.push(ServerTool::WebSearch);
    }
    tools.push(ServerTool::WebFetch);
    tools
}

fn off(capabilities: &Value) -> Option<ThinkingOff> {
    let types = &capabilities["thinking"]["types"];
    if supported(&types["disabled"]) {
        Some(ThinkingOff::Disabled)
    } else if supported(&types["between_tools"]) {
        Some(ThinkingOff::BetweenTools)
    } else {
        None
    }
}

fn supported(capability: &Value) -> bool {
    capability.get("supported").and_then(Value::as_bool) == Some(true)
}

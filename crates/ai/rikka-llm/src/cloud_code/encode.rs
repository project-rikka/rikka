use std::collections::{HashMap, VecDeque};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Map, Value, json};

use crate::cloud_code::schema;
use crate::content::Content;
use crate::deferral::Deferral;
use crate::document::{Document, DocumentSource};
use crate::effort::{Effort, Reveal};
use crate::message::Message;
use crate::random;
use crate::request::Request;
use crate::role::Role;
use crate::tool_choice::ToolChoice;
use crate::tool_result::{Output, ToolResult};
use crate::tool_use::ToolUse;
use crate::vendor::Vendor;
use rikka_tool::Media;
use rikka_tool::Tool;
use rikka_tool::{Image, ImageSource};

const UNSIGNED: &str = "skip_thought_signature_validator";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Family {
    Gemini,
    Claude,
}

impl Family {
    pub(crate) fn from_model(model: &str) -> Family {
        match model.to_ascii_lowercase().contains("claude") {
            true => Family::Claude,
            false => Family::Gemini,
        }
    }
}

pub(crate) fn body(request: &Request, model: &str, project: &str, session: &str) -> Value {
    let mut envelope = Map::new();
    envelope.insert("model".to_owned(), json!(model));
    envelope.insert("project".to_owned(), json!(project));
    envelope.insert("userAgent".to_owned(), json!("antigravity"));
    envelope.insert("requestType".to_owned(), json!("agent"));
    envelope.insert(
        "requestId".to_owned(),
        json!(format!("agent-{}", random::uuid())),
    );
    envelope.insert("request".to_owned(), inner(request, model, session));

    Value::Object(envelope)
}

fn inner(request: &Request, model: &str, session: &str) -> Value {
    let family = Family::from_model(model);

    let mut inner = Map::new();
    inner.insert("contents".to_owned(), contents(request, family));

    if !request.system.is_empty() {
        let parts = request
            .system
            .iter()
            .map(|text| json!({ "text": text.as_str() }))
            .collect::<Vec<_>>();
        inner.insert(
            "systemInstruction".to_owned(),
            json!({ "role": "user", "parts": parts }),
        );
    }
    let declared = Deferral::new(request).eager();
    if !declared.is_empty() {
        inner.insert(
            "tools".to_owned(),
            json!([{ "functionDeclarations": declarations(&declared) }]),
        );
        inner.insert("toolConfig".to_owned(), tool_config(request, family));
    }
    if let Some(config) = generation(request, family) {
        inner.insert("generationConfig".to_owned(), config);
    }
    inner.insert(
        "sessionId".to_owned(),
        json!(request.cache_key.as_deref().unwrap_or(session)),
    );

    Value::Object(inner)
}

#[derive(Debug, Default)]
struct Pairing<'a> {
    issued: usize,
    unanswered: HashMap<&'a str, VecDeque<IssuedCall<'a>>>,
}

#[derive(Debug, Clone)]
struct IssuedCall<'a> {
    id: String,
    name: &'a str,
}

impl<'a> Pairing<'a> {
    fn issue(&mut self, call: &'a ToolUse) -> IssuedCall<'a> {
        self.issued += 1;
        let request = IssuedCall {
            id: format!("call_{}", self.issued),
            name: call.name(),
        };

        self.unanswered
            .entry(call.id())
            .or_default()
            .push_back(request.clone());
        request
    }

    fn pair_result(&mut self, result: &ToolResult) -> Option<IssuedCall<'a>> {
        self.unanswered.get_mut(result.id())?.pop_front()
    }
}

fn contents(request: &Request, family: Family) -> Value {
    let mut pairing = Pairing::default();
    let mut turns = request
        .messages
        .iter()
        .filter_map(|message| turn(message, &mut pairing, family))
        .collect::<Vec<_>>();

    let opens_with_model = turns
        .first()
        .and_then(|turn| turn.get("role"))
        .and_then(Value::as_str)
        == Some("model");

    if family == Family::Gemini && opens_with_model {
        turns.insert(0, json!({ "role": "user", "parts": [{ "text": "" }] }));
    }
    Value::Array(turns)
}

fn turn<'a>(message: &'a Message, pairing: &mut Pairing<'a>, family: Family) -> Option<Value> {
    let role = match message.role() {
        Role::Assistant => "model",
        Role::User | Role::System => "user",
    };

    let mut parts = message
        .content()
        .iter()
        .filter_map(|content| part(content, pairing, family))
        .collect::<Vec<_>>();

    if family == Family::Gemini {
        sign(&mut parts);
    }
    (!parts.is_empty()).then(|| json!({ "role": role, "parts": parts }))
}

fn part<'a>(content: &'a Content, pairing: &mut Pairing<'a>, family: Family) -> Option<Value> {
    match content {
        Content::Text(text) => Some(json!({ "text": text.as_str() })),
        Content::Image(image) => Some(inline_image(image)),
        Content::Document(document) => Some(inline_document(document)),
        Content::ToolUse(call) => Some(function_call(call, pairing.issue(call), family)),
        Content::ToolResult(result) => pairing
            .pair_result(result)
            .map(|call| answer(result, &call)),
        Content::Reasoning(reasoning) => reasoning.replay(Vendor::Antigravity).cloned(),
        Content::Hosted(_) => None,
    }
}

fn function_call(call: &ToolUse, issued: IssuedCall<'_>, family: Family) -> Value {
    let mut part = json!({
        "functionCall": {
            "id": issued.id,
            "name": issued.name,
            "args": call.input(),
        }
    });

    if family == Family::Gemini
        && let Some(signature) = call.signature(Vendor::Antigravity)
    {
        part["thoughtSignature"] = json!(signature);
    }
    part
}

fn sign(parts: &mut [Value]) {
    if let Some(call) = parts
        .iter_mut()
        .find(|part| part.get("functionCall").is_some())
        && call.get("thoughtSignature").is_none()
    {
        call["thoughtSignature"] = json!(UNSIGNED);
    }
}

fn inline_image(image: &Image) -> Value {
    match image.source() {
        ImageSource::Inline { media, data } => json!({
            "inlineData": { "mimeType": media.mime(), "data": STANDARD.encode(data) }
        }),
        ImageSource::Url(url) => json!({
            "fileData": { "mimeType": Media::Png.mime(), "fileUri": url }
        }),
    }
}

fn inline_document(document: &Document) -> Value {
    match document.source() {
        DocumentSource::Inline { media, data } if *media == Media::PlainText => {
            json!({ "text": String::from_utf8_lossy(data) })
        }
        DocumentSource::Inline { media, data } => json!({
            "inlineData": { "mimeType": media.mime(), "data": STANDARD.encode(data) }
        }),
        DocumentSource::Url(url) => json!({
            "fileData": { "mimeType": Media::Pdf.mime(), "fileUri": url }
        }),
    }
}

fn answer(result: &ToolResult, call: &IssuedCall<'_>) -> Value {
    let mut response = json!({
        "id": call.id,
        "name": call.name,
        "response": { "result": result.text() },
    });

    let pictures = result
        .output()
        .iter()
        .filter_map(|output| match output {
            Output::Image(image) => Some(inline_image(image)),
            Output::Text(_) | Output::Reference(_) => None,
        })
        .collect::<Vec<_>>();

    if !pictures.is_empty() {
        response["parts"] = Value::Array(pictures);
    }
    json!({ "functionResponse": response })
}

fn declarations(tools: &[&Tool]) -> Value {
    let declarations = tools
        .iter()
        .map(|tool| {
            json!({
                "name": tool.name(),
                "description": tool.description(),
                "parameters": schema::gemini(tool.schema()),
            })
        })
        .collect::<Vec<_>>();

    Value::Array(declarations)
}

fn tool_config(request: &Request, family: Family) -> Value {
    let mut config = Map::new();

    let mode = match &request.tool_choice {
        ToolChoice::Auto => "AUTO",
        ToolChoice::Any | ToolChoice::Named(_) => "ANY",
        ToolChoice::None => "NONE",
    };
    config.insert("mode".to_owned(), json!(mode));

    if let ToolChoice::Named(name) = &request.tool_choice {
        config.insert("allowedFunctionNames".to_owned(), json!([name]));
    }
    if family == Family::Claude {
        config.insert("mode".to_owned(), json!("VALIDATED"));
    }

    json!({ "functionCallingConfig": Value::Object(config) })
}

fn generation(request: &Request, family: Family) -> Option<Value> {
    let mut config = Map::new();

    if let Some(temperature) = request.temperature {
        config.insert("temperature".to_owned(), json!(temperature));
    }
    if let Some(top_p) = request.top_p {
        config.insert("topP".to_owned(), json!(top_p));
    }
    if !request.stop.is_empty() {
        config.insert("stopSequences".to_owned(), json!(request.stop));
    }
    if let (Some(tokens), Family::Claude) = (request.max_tokens, family) {
        config.insert("maxOutputTokens".to_owned(), json!(tokens));
    }
    if let Some(thinking) = thinking(request, family) {
        config.insert("thinkingConfig".to_owned(), thinking);
    }
    if let Some(format) = &request.format {
        config.insert("responseMimeType".to_owned(), json!("application/json"));
        config.insert(
            "responseSchema".to_owned(),
            schema::gemini(&format.schema()),
        );
    }

    (!config.is_empty()).then_some(Value::Object(config))
}

fn thinking(request: &Request, family: Family) -> Option<Value> {
    let mut config = Map::new();

    match (request.effort, family) {
        (Some(Effort::Off), _) => return None,
        (Some(effort), Family::Gemini) => {
            config.insert("thinkingLevel".to_owned(), json!(level(effort)));
        }
        (Some(effort), Family::Claude) => {
            config.insert("thinkingBudget".to_owned(), json!(budget(effort, request)));
        }
        (None, _) => {}
    }

    if request.reveal == Reveal::Summary {
        config.insert("includeThoughts".to_owned(), json!(true));
    }
    (!config.is_empty()).then_some(Value::Object(config))
}

fn level(effort: Effort) -> &'static str {
    match effort {
        Effort::Off | Effort::Minimal => "minimal",
        Effort::Low => "low",
        Effort::Medium => "medium",
        Effort::High => "high",
        Effort::XHigh | Effort::Max => "xhigh",
    }
}

fn budget(effort: Effort, request: &Request) -> u32 {
    let thinking = match effort {
        Effort::Off => 0,
        Effort::Minimal => 1_024,
        Effort::Low => 4_096,
        Effort::Medium => 16_384,
        Effort::High => 32_768,
        Effort::XHigh => 49_152,
        Effort::Max => 63_000,
    };

    match request.max_tokens {
        Some(ceiling) => thinking.min(ceiling.saturating_sub(1_024).max(1_024)),
        None => thinking,
    }
}

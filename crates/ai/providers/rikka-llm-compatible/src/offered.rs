use rikka_llm::{Effort, ModelInfo};
use serde_json::Value;

const DEFAULT_CONTEXT: u64 = 32_768;
const DEFAULT_OUTPUT: u32 = 8_192;
const REASONING: [Effort; 3] = [Effort::Low, Effort::Medium, Effort::High];

const CONTEXT: &[&[&str]] = &[
    &["context_length"],
    &["context_window"],
    &["max_model_len"],
    &["max_context_length"],
    &["loaded_context_length"],
    &["top_provider", "context_length"],
    &["meta", "n_ctx"],
    &["meta", "n_ctx_train"],
    &["metadata", "context_length"],
];

const OUTPUT: &[&[&str]] = &[
    &["max_completion_tokens"],
    &["max_output_tokens"],
    &["top_provider", "max_completion_tokens"],
];

const EMBEDDING: [&str; 2] = ["embedding", "embeddings"];
const CHATTING: [&str; 3] = ["completion", "chat", "completion_chat"];

struct Capability {
    parameters: &'static [&'static str],
    listed: &'static [&'static str],
    flags: &'static [&'static str],
}

const TOOLS: Capability = Capability {
    parameters: &["tools", "tool_choice"],
    listed: &["tools", "tool_use", "function_calling"],
    flags: &[
        "function_calling",
        "tool_use",
        "tools",
        "supports_tools",
        "supports_function_calling",
    ],
};

const THINKING: Capability = Capability {
    parameters: &["reasoning", "reasoning_effort", "include_reasoning"],
    listed: &["thinking", "reasoning"],
    flags: &["reasoning", "thinking", "supports_reasoning"],
};

const VISION: [&str; 3] = ["vision", "image", "images"];
const VISION_FLAGS: &[&str] = &["vision", "supports_vision", "supports_image_input"];
const VISION_KIND: &str = "vlm";

pub(crate) struct Described {
    pub(crate) context: Option<u64>,
    pub(crate) vision: Option<bool>,
    pub(crate) tools: Option<bool>,
    pub(crate) reasoning: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Offered {
    id: String,
    name: String,
    context: Option<u64>,
    output: Option<u32>,
    vision: Option<bool>,
    tools: Option<bool>,
    reasoning: Option<bool>,
}

impl Offered {
    #[must_use]
    pub fn unlisted(id: &str) -> Offered {
        let id = id.trim();
        Offered {
            id: id.to_owned(),
            name: id.to_owned(),
            ..Offered::default()
        }
    }

    pub(crate) fn read(entry: &Value) -> Option<Offered> {
        let id = first_text(entry, &["id", "model", "name"])?;
        if is_embedding(entry) {
            return None;
        }
        let name = first_text(entry, &["display_name", "name"]).unwrap_or_else(|| id.clone());
        Some(Offered {
            context: first_count(entry, CONTEXT),
            output: first_count(entry, OUTPUT)
                .map(|tokens| u32::try_from(tokens).unwrap_or(u32::MAX)),
            vision: vision(entry),
            tools: capability(entry, &TOOLS),
            reasoning: capability(entry, &THINKING),
            id,
            name,
        })
    }

    pub(crate) fn described(self, described: Described) -> Offered {
        Offered {
            context: described.context.or(self.context),
            vision: described.vision.or(self.vision),
            tools: described.tools.or(self.tools),
            reasoning: described.reasoning.or(self.reasoning),
            ..self
        }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn context(&self) -> Option<u64> {
        self.context
    }

    #[must_use]
    pub fn output(&self) -> Option<u32> {
        self.output
    }

    #[must_use]
    pub fn vision(&self) -> Option<bool> {
        self.vision
    }

    #[must_use]
    pub fn tools(&self) -> Option<bool> {
        self.tools
    }

    #[must_use]
    pub fn reasoning(&self) -> Option<bool> {
        self.reasoning
    }

    #[must_use]
    pub fn model_info(&self) -> ModelInfo {
        let context = self.context.unwrap_or(DEFAULT_CONTEXT);
        let output = self.output.unwrap_or_else(|| {
            u32::try_from(context).map_or(DEFAULT_OUTPUT, |context| context.min(DEFAULT_OUTPUT))
        });
        let efforts = match self.reasoning {
            Some(true) => REASONING.to_vec(),
            Some(false) | None => Vec::new(),
        };
        ModelInfo::new(&self.id, &self.name, context, output)
            .with_vision(self.vision.unwrap_or(false))
            .with_tools(self.tools.unwrap_or(true))
            .with_efforts(efforts)
    }
}

fn first_text(entry: &Value, fields: &[&str]) -> Option<String> {
    fields.iter().find_map(|field| {
        entry
            .get(field)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_owned)
    })
}

fn first_count(entry: &Value, paths: &[&[&str]]) -> Option<u64> {
    paths.iter().find_map(|path| {
        path.iter()
            .try_fold(entry, |value, field| value.get(field))
            .and_then(Value::as_u64)
            .filter(|count| *count > 0)
    })
}

fn is_embedding(entry: &Value) -> bool {
    let kind = entry
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if EMBEDDING.contains(&kind) {
        return true;
    }
    let listed = listed(entry);
    listed.iter().any(|named| EMBEDDING.contains(named))
        && !listed.iter().any(|named| CHATTING.contains(named))
}

fn listed(entry: &Value) -> Vec<&str> {
    entry
        .get("capabilities")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter_map(Value::as_str)
        .collect()
}

fn flagged(entry: &Value, flags: &[&str]) -> Option<bool> {
    let held = [Some(entry), entry.get("capabilities")];
    flags.iter().find_map(|flag| {
        held.iter()
            .flatten()
            .find_map(|holder| holder.get(flag).and_then(Value::as_bool))
    })
}

fn capability(entry: &Value, capability: &Capability) -> Option<bool> {
    let parameters = entry.get("supported_parameters").and_then(Value::as_array);
    if let Some(parameters) = parameters {
        return Some(
            parameters
                .iter()
                .filter_map(Value::as_str)
                .any(|named| capability.parameters.contains(&named)),
        );
    }
    let listed = listed(entry);
    if !listed.is_empty() {
        return Some(listed.iter().any(|named| capability.listed.contains(named)));
    }
    flagged(entry, capability.flags)
}

fn vision(entry: &Value) -> Option<bool> {
    let modalities = entry
        .get("architecture")
        .and_then(|architecture| architecture.get("input_modalities"))
        .or_else(|| entry.get("input_modalities"))
        .or_else(|| {
            entry
                .get("modalities")
                .and_then(|modalities| modalities.get("input"))
        })
        .and_then(Value::as_array);
    if let Some(modalities) = modalities {
        return Some(
            modalities
                .iter()
                .any(|modality| modality.as_str() == Some("image")),
        );
    }
    if entry.get("type").and_then(Value::as_str) == Some(VISION_KIND) {
        return Some(true);
    }
    let listed = listed(entry);
    if !listed.is_empty() {
        return Some(listed.iter().any(|named| VISION.contains(named)));
    }
    flagged(entry, VISION_FLAGS)
}

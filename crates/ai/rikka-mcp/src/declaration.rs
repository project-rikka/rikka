use std::collections::BTreeSet;

use rikka_llm::{Tool, Value, json};

use crate::naming;
use crate::offer::Offer;
use crate::protocol::Tool as Offered;

#[derive(Clone, Debug)]
pub(crate) struct Declared {
    tool: Tool,
    offer: Offer,
}

impl Declared {
    pub(crate) fn tool(&self) -> &Tool {
        &self.tool
    }

    pub(crate) fn called(&self) -> &str {
        self.offer.name()
    }

    pub(crate) fn offer(&self) -> &Offer {
        &self.offer
    }
}

pub(crate) fn declared(label: &str, tools: Vec<Offered>) -> Vec<Declared> {
    let mut used_names = BTreeSet::new();

    tools
        .into_iter()
        .map(|tool| {
            let description = tool_description(&tool);
            Declared {
                tool: Tool::from_schema(
                    naming::unique(naming::named(label, &tool.name), &mut used_names),
                    shaped(&tool),
                )
                .description(described(label, &tool.name, description.as_deref()))
                .build()
                .expect("fully expanded schema should be self-contained"),
                offer: Offer::new(tool.name, description),
            }
        })
        .collect()
}

fn tool_description(tool: &Offered) -> Option<String> {
    tool.description
        .as_deref()
        .or(tool.title.as_deref())
        .or_else(|| {
            tool.annotations
                .as_ref()
                .and_then(|annotations| annotations.title.as_deref())
        })
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

fn described(label: &str, name: &str, description: Option<&str>) -> String {
    match description {
        Some(description) => description.to_owned(),
        None => format!(
            "The \"{name}\" tool of the \"{label}\" MCP server, which says nothing further about it."
        ),
    }
}

fn shaped(tool: &Offered) -> Value {
    let mut schema = tool.input_schema.clone();
    schema.entry("type").or_insert_with(|| json!("object"));
    schema.entry("properties").or_insert_with(|| json!({}));
    Value::Object(schema)
}

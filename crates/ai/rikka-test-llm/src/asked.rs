use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct Asked(Value);

#[derive(Debug, Clone, PartialEq)]
pub struct Turn(Value);

impl Asked {
    pub(crate) fn new(body: Value) -> Asked {
        Asked(body)
    }

    #[must_use]
    pub fn body(&self) -> &Value {
        &self.0
    }

    #[must_use]
    pub fn messages(&self) -> Vec<Turn> {
        listed(&self.0["messages"])
            .iter()
            .cloned()
            .map(Turn)
            .collect()
    }

    #[must_use]
    pub fn tools(&self) -> Vec<String> {
        listed(&self.0["tools"])
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .map(str::to_owned)
            .collect()
    }
}

impl Turn {
    #[must_use]
    pub fn role(&self) -> &str {
        self.0["role"].as_str().unwrap_or_default()
    }

    #[must_use]
    pub fn blocks(&self) -> usize {
        match &self.0["content"] {
            Value::String(_) => 1,
            content => listed(content).len(),
        }
    }

    #[must_use]
    pub fn text(&self) -> String {
        if let Some(text) = self.0["content"].as_str() {
            return text.to_owned();
        }
        self.of_kind("text", "text").join("\n")
    }

    #[must_use]
    pub fn tool_calls(&self) -> usize {
        self.of_kind("tool_use", "id").len()
    }

    #[must_use]
    pub fn thoughts(&self) -> Vec<String> {
        self.of_kind("thinking", "thinking")
    }

    fn of_kind(&self, kind: &str, field: &str) -> Vec<String> {
        listed(&self.0["content"])
            .iter()
            .filter(|block| block["type"] == kind)
            .filter_map(|block| block[field].as_str())
            .map(str::to_owned)
            .collect()
    }
}

fn listed(value: &Value) -> &[Value] {
    value.as_array().map_or(&[], Vec::as_slice)
}

use serde_json::{Map, Value, json};

#[derive(Debug, Clone, PartialEq)]
pub struct Schema {
    value: Value,
}

impl Schema {
    #[must_use]
    pub fn string() -> Schema {
        Schema::primitive("string")
    }

    #[must_use]
    pub fn integer() -> Schema {
        Schema::primitive("integer")
    }

    #[must_use]
    pub fn number() -> Schema {
        Schema::primitive("number")
    }

    #[must_use]
    pub fn boolean() -> Schema {
        Schema::primitive("boolean")
    }

    #[must_use]
    pub fn array(items: Schema) -> Schema {
        Schema {
            value: json!({ "type": "array", "items": items.value }),
        }
    }

    #[must_use]
    pub fn object() -> Schema {
        Schema {
            value: json!({ "type": "object", "properties": {}, "required": [] }),
        }
    }

    #[must_use]
    pub fn any_of(branches: impl IntoIterator<Item = Schema>) -> Schema {
        let branches = branches.into_iter().map(Schema::into_value).collect();
        Schema {
            value: json!({ "anyOf": Value::Array(branches) }),
        }
    }

    #[must_use]
    pub fn any() -> Schema {
        Schema { value: json!({}) }
    }

    #[must_use]
    pub fn raw(value: Value) -> Schema {
        Schema { value }
    }

    #[must_use]
    pub fn describe(mut self, description: impl Into<String>) -> Self {
        self.set("description", Value::String(description.into()));
        self
    }

    #[must_use]
    pub fn closed(mut self) -> Self {
        if let Some(object) = self.value.as_object_mut() {
            only(object);
        }
        self
    }

    #[must_use]
    pub fn enumerate<T: Into<Value>>(mut self, values: impl IntoIterator<Item = T>) -> Self {
        let values = values.into_iter().map(Into::into).collect::<Vec<_>>();
        self.set("enum", Value::Array(values));
        self
    }

    #[must_use]
    pub fn param(mut self, name: impl Into<String>, schema: Schema) -> Self {
        let name = name.into();
        self.declare(&name, schema);
        self.require(name);
        self
    }

    #[must_use]
    pub fn optional(mut self, name: impl Into<String>, schema: Schema) -> Self {
        self.declare(&name.into(), schema);
        self
    }

    #[must_use]
    pub fn into_value(self) -> Value {
        self.value
    }

    #[must_use]
    pub fn as_value(&self) -> &Value {
        &self.value
    }

    pub(crate) fn declares(&self, name: &str) -> bool {
        self.value
            .get("properties")
            .and_then(Value::as_object)
            .is_some_and(|properties| properties.contains_key(name))
    }

    pub(crate) fn demote(&mut self, name: &str) {
        if let Some(required) = self.value.get_mut("required").and_then(Value::as_array_mut) {
            required.retain(|entry| entry.as_str() != Some(name));
        }
    }

    pub fn close(&mut self) {
        close(&mut self.value);
    }

    fn primitive(kind: &str) -> Schema {
        Schema {
            value: json!({ "type": kind }),
        }
    }

    fn set(&mut self, key: &str, value: Value) {
        if let Some(object) = self.value.as_object_mut() {
            object.insert(key.to_owned(), value);
        }
    }

    fn declare(&mut self, name: &str, schema: Schema) {
        let Some(object) = self.value.as_object_mut() else {
            return;
        };
        object
            .entry("properties")
            .or_insert_with(|| Value::Object(Map::new()));

        if let Some(properties) = object["properties"].as_object_mut() {
            properties.insert(name.to_owned(), schema.value);
        }
    }

    fn require(&mut self, name: String) {
        let Some(object) = self.value.as_object_mut() else {
            return;
        };
        object
            .entry("required")
            .or_insert_with(|| Value::Array(Vec::new()));

        if let Some(required) = object["required"].as_array_mut() {
            required.push(Value::String(name));
        }
    }
}

fn close(value: &mut Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };

    for nested in object.values_mut() {
        match nested {
            Value::Object(_) => close(nested),
            Value::Array(entries) => entries.iter_mut().for_each(close),
            _ => {}
        }
    }

    if is_object(object) {
        only(object);
        require_all(object);
    }
}

fn only(object: &mut Map<String, Value>) {
    object.insert("additionalProperties".to_owned(), Value::Bool(false));
}

fn is_object(object: &Map<String, Value>) -> bool {
    match object.get("type") {
        Some(Value::String(kind)) => kind == "object",
        Some(Value::Array(kinds)) => kinds.iter().any(|kind| kind == "object"),
        _ => false,
    }
}

fn require_all(object: &mut Map<String, Value>) {
    let Some(properties) = object.get("properties").and_then(Value::as_object) else {
        return;
    };
    let names = properties.keys().cloned().collect::<Vec<_>>();

    let required = object
        .get("required")
        .and_then(Value::as_array)
        .map(|required| {
            required
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let optional = names
        .into_iter()
        .filter(|name| !required.contains(name))
        .collect::<Vec<_>>();

    if let Some(properties) = object.get_mut("properties").and_then(Value::as_object_mut) {
        for name in &optional {
            if let Some(property) = properties.get_mut(name) {
                nullable(property);
            }
        }
    }

    let all = required.into_iter().chain(optional).map(Value::String);
    object.insert("required".to_owned(), Value::Array(all.collect()));
}

fn nullable(property: &mut Value) {
    let Some(object) = property.as_object_mut() else {
        return;
    };

    let widened = match object.get("type") {
        Some(Value::String(kind)) if kind != "null" => {
            json!([kind, "null"])
        }
        Some(Value::Array(kinds)) if !kinds.iter().any(|kind| kind == "null") => {
            let mut kinds = kinds.clone();
            kinds.push(json!("null"));
            Value::Array(kinds)
        }
        _ => return,
    };

    object.insert("type".to_owned(), widened);
}

impl Default for Schema {
    fn default() -> Schema {
        Schema::object()
    }
}

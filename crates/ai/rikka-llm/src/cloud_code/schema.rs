use serde_json::{Map, Value, json};

const KEPT: &[&str] = &[
    "type",
    "format",
    "title",
    "description",
    "nullable",
    "enum",
    "minItems",
    "maxItems",
    "minProperties",
    "maxProperties",
    "minimum",
    "maximum",
    "minLength",
    "maxLength",
    "pattern",
    "example",
    "default",
    "propertyOrdering",
];

const DEPTH: usize = 8;

pub(crate) fn gemini(schema: &Value) -> Value {
    let defs = definitions(schema);
    clean(schema, &defs, DEPTH)
}

fn definitions(root: &Value) -> Map<String, Value> {
    let mut defs = Map::new();

    for field in ["$defs", "definitions"] {
        if let Some(Value::Object(named)) = root.get(field) {
            defs.extend(named.clone());
        }
    }
    defs
}

fn clean(schema: &Value, defs: &Map<String, Value>, depth: usize) -> Value {
    let Some(fields) = schema.as_object() else {
        return json!({});
    };

    if let Some(target) = reference(fields, defs, depth) {
        return target;
    }

    let mut out = Map::new();
    let mut nullable = fields.get("nullable").and_then(Value::as_bool) == Some(true);

    for (key, value) in fields {
        match key.as_str() {
            "type" => match value {
                Value::Array(names) => {
                    nullable |= names.iter().any(|name| name == "null");
                    if let Some(name) = names.iter().find(|name| *name != "null") {
                        out.insert("type".to_owned(), name.clone());
                    }
                }
                _ => {
                    out.insert("type".to_owned(), value.clone());
                }
            },
            "const" => {
                out.insert("enum".to_owned(), json!([value.clone()]));
            }
            "properties" => {
                if let Some(properties) = value.as_object() {
                    let cleaned = properties
                        .iter()
                        .map(|(name, schema)| (name.clone(), clean(schema, defs, depth)))
                        .collect::<Map<_, _>>();
                    out.insert("properties".to_owned(), Value::Object(cleaned));
                }
            }
            "items" => {
                out.insert("items".to_owned(), clean(value, defs, depth));
            }
            "required" => {
                out.insert("required".to_owned(), value.clone());
            }
            "anyOf" | "oneOf" => {
                let (branches, null) = union(value, defs, depth);
                nullable |= null;
                if !branches.is_empty() {
                    out.insert("anyOf".to_owned(), Value::Array(branches));
                }
            }
            "allOf" => merge(value, defs, depth, &mut out),
            key if KEPT.contains(&key) => {
                out.insert(key.to_owned(), value.clone());
            }
            _ => {}
        }
    }

    if nullable {
        out.insert("nullable".to_owned(), json!(true));
    }
    if let Some(Value::Array(branches)) = out.get("anyOf")
        && branches.len() == 1
        && out.len() == 1
    {
        return branches[0].clone();
    }
    Value::Object(out)
}

fn union(value: &Value, defs: &Map<String, Value>, depth: usize) -> (Vec<Value>, bool) {
    let Some(branches) = value.as_array() else {
        return (Vec::new(), false);
    };

    let mut nullable = false;
    let cleaned = branches
        .iter()
        .filter(|branch| match branch.get("type") {
            Some(Value::String(name)) if name == "null" => {
                nullable = true;
                false
            }
            _ => true,
        })
        .map(|branch| clean(branch, defs, depth))
        .collect();

    (cleaned, nullable)
}

fn merge(value: &Value, defs: &Map<String, Value>, depth: usize, out: &mut Map<String, Value>) {
    let Some(branches) = value.as_array() else {
        return;
    };

    for branch in branches {
        let Value::Object(cleaned) = clean(branch, defs, depth) else {
            continue;
        };

        for (key, value) in cleaned {
            match (out.get_mut(&key), value) {
                (Some(Value::Object(into)), Value::Object(from)) => into.extend(from),
                (Some(Value::Array(into)), Value::Array(from)) => into.extend(from),
                (Some(_), _) => {}
                (None, value) => {
                    out.insert(key, value);
                }
            }
        }
    }
}

fn reference(
    fields: &Map<String, Value>,
    defs: &Map<String, Value>,
    depth: usize,
) -> Option<Value> {
    let path = fields.get("$ref")?.as_str()?;
    if depth == 0 {
        return Some(json!({}));
    }

    let name = path
        .strip_prefix("#/$defs/")
        .or_else(|| path.strip_prefix("#/definitions/"))?;

    Some(match defs.get(name) {
        Some(target) => clean(target, defs, depth - 1),
        None => json!({}),
    })
}

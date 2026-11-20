use serde_json::Value;

const SEPARATOR: &str = ", ";

pub(crate) fn gist(paths: &[String], input: &Value) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    for path in paths {
        let mut values = vec![input];
        for step in path.split('.') {
            let (key, spread) = match step.strip_suffix("[]") {
                Some(key) => (key, true),
                None => (step, false),
            };
            values = values
                .into_iter()
                .filter_map(|value| value.get(key))
                .flat_map(|value| match (spread, value) {
                    (true, Value::Array(items)) => items.iter().collect::<Vec<&Value>>(),
                    (true, _) => Vec::new(),
                    (false, value) => vec![value],
                })
                .collect();
        }
        parts.extend(values.into_iter().filter_map(as_text));
    }
    (!parts.is_empty()).then(|| parts.join(SEPARATOR))
}

fn as_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) if !text.trim().is_empty() => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

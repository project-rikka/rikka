use serde_json::Value;

pub(crate) trait Walk {
    fn at(&self, path: &[&str]) -> Option<&Value>;

    fn text_at(&self, path: &[&str]) -> Option<&str>;

    fn list(&self, path: &[&str]) -> &[Value];

    fn runs(&self, path: &[&str]) -> &[Value];

    fn message(&self, path: &[&str]) -> Option<String>;

    fn every(&self, key: &str) -> Vec<&Value>;

    fn first(&self, key: &str) -> Option<&Value>;
}

impl Walk for Value {
    fn at(&self, path: &[&str]) -> Option<&Value> {
        path.iter()
            .try_fold(self, |node, key| match key.parse::<usize>() {
                Ok(index) => node.get(index),
                Err(_) => node.get(key),
            })
    }

    fn text_at(&self, path: &[&str]) -> Option<&str> {
        self.at(path)?.as_str()
    }

    fn list(&self, path: &[&str]) -> &[Value] {
        self.at(path)
            .and_then(Value::as_array)
            .map_or(&[], Vec::as_slice)
    }

    fn runs(&self, path: &[&str]) -> &[Value] {
        self.at(path)
            .and_then(|node| node.get("runs"))
            .and_then(Value::as_array)
            .map_or(&[], Vec::as_slice)
    }

    fn message(&self, path: &[&str]) -> Option<String> {
        let node = self.at(path)?;
        if let Some(text) = node.as_str() {
            return Some(text.to_owned());
        }
        if let Some(text) = node.get("simpleText").and_then(Value::as_str) {
            return Some(text.to_owned());
        }
        let text: String = node
            .get("runs")?
            .as_array()?
            .iter()
            .filter_map(|run| run.get("text").and_then(Value::as_str))
            .collect();
        (!text.is_empty()).then_some(text)
    }

    fn every(&self, key: &str) -> Vec<&Value> {
        let mut values = Vec::new();
        collect_values(self, key, &mut values);
        values
    }

    fn first(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(map) => map.iter().find_map(|(name, value)| match name == key {
                true => Some(value),
                false => value.first(key),
            }),
            Value::Array(items) => items.iter().find_map(|item| item.first(key)),
            _ => None,
        }
    }
}

fn collect_values<'a>(node: &'a Value, key: &str, values: &mut Vec<&'a Value>) {
    match node {
        Value::Object(map) => {
            for (name, value) in map {
                match name == key {
                    true => values.push(value),
                    false => collect_values(value, key, values),
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_values(item, key, values);
            }
        }
        _ => {}
    }
}

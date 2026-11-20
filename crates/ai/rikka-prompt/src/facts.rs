use std::collections::BTreeMap;

#[derive(Clone, Debug, Default)]
pub struct Facts {
    values: BTreeMap<String, String>,
}

impl Facts {
    #[must_use]
    pub fn new() -> Facts {
        Facts::default()
    }

    pub fn set(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.values.insert(name.into(), value.into());
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }
}

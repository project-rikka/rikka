use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Filter {
    pub label: String,
    pub rules: Vec<Rule>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "by", content = "pattern")]
pub enum Rule {
    Glob(String),
    Mime(String),
}

impl Filter {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            rules: Vec::new(),
        }
    }

    #[must_use]
    pub fn with(mut self, rule: Rule) -> Self {
        self.rules.push(rule);
        self
    }

    #[must_use]
    pub fn matches(&self, name: &str, types: &[&str]) -> bool {
        self.rules.is_empty()
            || self.rules.iter().any(|rule| match rule {
                Rule::Glob(pattern) => rikka_mime::matches_glob_folding_case(pattern, name),
                Rule::Mime(pattern) => types
                    .iter()
                    .any(|kind| rikka_mime::matches_glob_folding_case(pattern, kind)),
            })
    }
}

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Origin {
    pub host: String,
    pub user: String,
    pub home: String,
    pub created: u64,
}

impl Origin {
    #[must_use]
    pub fn rebase(&self, value: &str, home: &str) -> String {
        let from = self.home.trim_end_matches('/');
        if from.is_empty() {
            return value.to_owned();
        }
        match value.strip_prefix(from) {
            Some("") => home.to_owned(),
            Some(rest) if rest.starts_with('/') => format!("{}{rest}", home.trim_end_matches('/')),
            _ => value.to_owned(),
        }
    }
}

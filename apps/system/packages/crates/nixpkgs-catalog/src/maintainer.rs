use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct Maintainer {
    pub name: String,
    pub github: String,
    pub email: String,
}

impl Maintainer {
    #[must_use]
    pub fn label(&self) -> Option<&str> {
        [&self.name, &self.github, &self.email]
            .into_iter()
            .map(String::as_str)
            .find(|name| !name.is_empty())
    }
}

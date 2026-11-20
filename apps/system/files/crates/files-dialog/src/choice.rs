use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Choice {
    pub id: String,
    pub label: String,
    pub options: Vec<ChoiceOption>,
    pub initial: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChoiceOption {
    pub id: String,
    pub label: String,
}

pub const ON: &str = "true";
pub const OFF: &str = "false";

impl Choice {
    #[must_use]
    pub fn is_switch(&self) -> bool {
        self.options.is_empty()
    }

    #[must_use]
    pub fn initial_index(&self) -> usize {
        self.options
            .iter()
            .position(|option| option.id == self.initial)
            .unwrap_or(0)
    }

    #[must_use]
    pub fn starts_on(&self) -> bool {
        self.initial == ON
    }
}

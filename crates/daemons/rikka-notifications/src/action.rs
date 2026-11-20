pub const DEFAULT_ACTION: &str = "default";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Action {
    pub key: String,
    pub label: String,
}

impl Action {
    #[must_use]
    pub fn is_default(&self) -> bool {
        self.key == DEFAULT_ACTION
    }
}

#[must_use]
pub(crate) fn read(flat: Vec<String>) -> Vec<Action> {
    let mut actions = Vec::with_capacity(flat.len() / 2);
    let mut pairs = flat.into_iter();
    while let (Some(key), Some(label)) = (pairs.next(), pairs.next()) {
        if !key.is_empty() {
            actions.push(Action { key, label });
        }
    }
    actions
}

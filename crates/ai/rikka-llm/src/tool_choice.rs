#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ToolChoice {
    #[default]
    Auto,
    Any,
    Named(String),
    None,
}

impl ToolChoice {
    #[must_use]
    pub fn named(name: impl Into<String>) -> ToolChoice {
        ToolChoice::Named(name.into())
    }
}

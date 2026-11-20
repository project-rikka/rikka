#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThinkingOff {
    #[default]
    Disabled,
    BetweenTools,
}

impl ThinkingOff {
    #[must_use]
    pub fn is_disabled(&self) -> bool {
        *self == ThinkingOff::Disabled
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Stop {
    #[default]
    End,
    ToolUse,
    MaxTokens,
    Paused,
    Sequence(String),
    Declined(String),
    Filtered,
}

impl Stop {
    #[must_use]
    pub fn wants_tools(&self) -> bool {
        matches!(self, Stop::ToolUse)
    }

    #[must_use]
    pub fn is_paused(&self) -> bool {
        matches!(self, Stop::Paused)
    }

    #[must_use]
    pub fn is_complete(&self) -> bool {
        matches!(self, Stop::End | Stop::ToolUse | Stop::Sequence(_))
    }
}

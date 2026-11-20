use rikka_text_unicode::Script;

use crate::common::direction::Direction;
use crate::common::language::Language;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SegmentProperties {
    pub direction: Direction,
    pub script: Script,
    pub language: Option<Language>,
}

impl SegmentProperties {
    pub fn new(script: Script) -> Self {
        Self {
            direction: Direction::from_script(script).unwrap_or_default(),
            script,
            language: None,
        }
    }

    #[must_use]
    pub fn with_direction(mut self, direction: Direction) -> Self {
        self.direction = direction;
        self
    }

    #[must_use]
    pub fn with_language(mut self, language: impl Into<Option<Language>>) -> Self {
        self.language = language.into();
        self
    }
}

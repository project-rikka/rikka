#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Text {
    text: String,
    cached: bool,
}

impl Text {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Text {
        Text {
            text: text.into(),
            cached: false,
        }
    }

    #[must_use]
    pub fn cached(mut self) -> Self {
        self.cached = true;
        self
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn is_cached(&self) -> bool {
        self.cached
    }
}

impl From<&str> for Text {
    fn from(text: &str) -> Text {
        Text::new(text)
    }
}

impl From<String> for Text {
    fn from(text: String) -> Text {
        Text::new(text)
    }
}

impl std::fmt::Display for Text {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.text)
    }
}

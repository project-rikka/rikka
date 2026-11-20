use std::sync::Arc;

use crate::args::Args;
use crate::parser::{self, SyntaxError};
use crate::pattern::Pattern;

#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    text: Arc<str>,
    pattern: Option<Pattern>,
}

impl Message {
    pub fn parse(source: &str) -> Result<Self, SyntaxError> {
        if !source.contains(['{', '}', '\'']) {
            return Ok(Self {
                text: Arc::from(source),
                pattern: None,
            });
        }

        let pattern = parser::parse(source)?;
        Ok(match pattern.as_literal() {
            Some(text) => Self {
                text: Arc::from(text),
                pattern: None,
            },
            None => Self {
                text: Arc::from(source),
                pattern: Some(pattern),
            },
        })
    }

    pub fn as_literal(&self) -> Option<&Arc<str>> {
        self.pattern.is_none().then_some(&self.text)
    }

    pub fn source(&self) -> &str {
        &self.text
    }

    pub fn format(&self, language: &str, args: &Args<'_>) -> Arc<str> {
        let Some(pattern) = &self.pattern else {
            return self.text.clone();
        };
        let mut out = String::with_capacity(self.text.len());
        pattern
            .format(language, args, None, &mut out)
            .expect("formatting into a String is infallible");
        Arc::from(out)
    }
}

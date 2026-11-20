use rikka_llm::Content;

use crate::attachment::Attachment;
use crate::turn::{TurnText, asked_amid};

const BETWEEN: &str = "\n\n";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ask {
    text: String,
    expanded: Option<String>,
    context: Vec<String>,
    attachments: Vec<Attachment>,
}

impl Ask {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Ask {
        Ask {
            text: text.into(),
            ..Ask::default()
        }
    }

    #[must_use]
    pub fn join(asks: impl IntoIterator<Item = Ask>) -> Option<Ask> {
        let mut asks: Vec<Ask> = asks.into_iter().filter(|ask| !ask.is_empty()).collect();
        if asks.len() <= 1 {
            return asks.pop();
        }
        let mut joined = Ask::new(joined(asks.iter().map(Ask::sent_text)));
        for ask in asks {
            joined.context.extend(ask.context);
            joined.attachments.extend(ask.attachments);
        }
        Some(joined)
    }

    #[must_use]
    pub fn with_expansion(mut self, sent: impl Into<String>) -> Ask {
        self.expanded = Some(sent.into());
        self
    }

    #[must_use]
    pub fn with_context(mut self, context: Vec<String>) -> Ask {
        self.context = context;
        self
    }

    #[must_use]
    pub fn with_attachments(mut self, attachments: Vec<Attachment>) -> Ask {
        self.attachments = attachments;
        self
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn sent_text(&self) -> &str {
        self.expanded.as_deref().unwrap_or(&self.text)
    }

    #[must_use]
    pub fn attachments(&self) -> &[Attachment] {
        &self.attachments
    }

    #[must_use]
    pub fn into_draft(self) -> (String, Vec<Attachment>) {
        (self.text, self.attachments)
    }

    pub(crate) fn led_by(mut self, restated: Option<String>) -> Ask {
        if let Some(restated) = restated {
            self.context.insert(0, restated);
        }
        self
    }

    pub(crate) fn content(&self) -> Vec<Content> {
        asked_amid(&self.context, self.sent_text(), &self.attachments)
    }

    pub(crate) fn into_turn_text(
        self,
        instructions: Option<Vec<String>>,
    ) -> (TurnText, Vec<Attachment>) {
        let reply = TurnText {
            text: self.text,
            expanded: self.expanded,
            context: self.context,
            instructions,
            ..TurnText::default()
        };
        (reply, self.attachments)
    }

    fn is_empty(&self) -> bool {
        self.sent_text().trim().is_empty() && self.context.is_empty() && self.attachments.is_empty()
    }
}
impl<T: Into<String>> From<T> for Ask {
    fn from(text: T) -> Ask {
        Ask::new(text)
    }
}

fn joined<'a>(parts: impl Iterator<Item = &'a str>) -> String {
    parts
        .filter(|parts| !parts.trim().is_empty())
        .collect::<Vec<&str>>()
        .join(BETWEEN)
}

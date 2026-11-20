use std::sync::Arc;

use rikka_llm::{Content, Hosted, Message, Reasoning, ToolUse, Value};

use crate::round::worth;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Replay {
    pieces: Arc<[Piece]>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Piece {
    Text {
        text: String,
    },
    Thought {
        reasoning: Reasoning,
    },
    Call {
        id: String,
        tool: String,
        input: Value,
    },
    Hosted {
        hosted: Hosted,
    },
}

impl Replay {
    pub(crate) fn from_message(message: Message) -> Replay {
        let pieces = message
            .into_content()
            .into_iter()
            .filter_map(|content| match content {
                Content::Text(text) => Some(Piece::Text {
                    text: text.as_str().to_owned(),
                }),
                Content::Reasoning(reasoning) => Some(Piece::Thought { reasoning }),
                Content::ToolUse(call) => Some(Piece::Call {
                    id: call.id().to_owned(),
                    tool: call.name().to_owned(),
                    input: call.input().clone(),
                }),
                Content::Hosted(hosted) => Some(Piece::Hosted { hosted }),
                _ => None,
            })
            .collect();
        Replay { pieces }
    }

    pub(crate) fn then(&self, more: &Replay) -> Replay {
        Replay {
            pieces: self
                .pieces
                .iter()
                .chain(more.pieces.iter())
                .cloned()
                .collect(),
        }
    }

    pub(crate) fn requested_ids(&self) -> impl Iterator<Item = &str> {
        self.pieces.iter().filter_map(|piece| match piece {
            Piece::Call { id, .. } => Some(id.as_str()),
            _ => None,
        })
    }

    pub(crate) fn content(
        &self,
        thinking: bool,
        is_requested: impl Fn(&str) -> bool,
    ) -> Vec<Content> {
        self.pieces
            .iter()
            .filter_map(|piece| match piece {
                Piece::Text { text } => Some(Content::text(text.as_str())),
                Piece::Thought { reasoning } => {
                    thinking.then(|| Content::Reasoning(reasoning.clone()))
                }
                Piece::Call { id, tool, input } => is_requested(id)
                    .then(|| Content::ToolUse(ToolUse::new(id, tool, input.clone()))),
                Piece::Hosted { hosted } => Some(Content::Hosted(hosted.clone())),
            })
            .collect()
    }

    pub(crate) fn bytes(&self) -> usize {
        self.pieces
            .iter()
            .map(|piece| match piece {
                Piece::Text { text } => text.len(),
                Piece::Thought { reasoning } => reasoning.text().len(),
                Piece::Call { id, tool, input } => id.len() + tool.len() + worth(input),
                Piece::Hosted { hosted } => hosted.bytes(),
            })
            .sum()
    }
}

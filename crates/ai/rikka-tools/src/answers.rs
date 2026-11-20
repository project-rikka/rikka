use serde::{Deserialize, Serialize};

use crate::question::Question;
use crate::selection::Selection;

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct Answers {
    #[serde(default)]
    answers: Vec<AnsweredQuestion>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct AnsweredQuestion {
    #[serde(default)]
    header: String,
    question: String,
    #[serde(flatten)]
    selection: Selection,
}

impl Answers {
    #[must_use]
    pub fn new(answers: Vec<AnsweredQuestion>) -> Answers {
        Answers { answers }
    }

    #[must_use]
    pub fn read(result: &str) -> Answers {
        serde_json::from_str(result).unwrap_or_default()
    }

    #[must_use]
    pub fn as_slice(&self) -> &[AnsweredQuestion] {
        &self.answers
    }
}

impl AnsweredQuestion {
    #[must_use]
    pub fn new(question: &Question, selection: Selection) -> AnsweredQuestion {
        AnsweredQuestion {
            header: question.header().to_owned(),
            question: question.text().to_owned(),
            selection,
        }
    }

    #[must_use]
    pub fn header(&self) -> &str {
        &self.header
    }

    #[must_use]
    pub fn question(&self) -> &str {
        &self.question
    }

    #[must_use]
    pub fn selection(&self) -> &Selection {
        &self.selection
    }
}

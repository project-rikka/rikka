use std::ops::Range;
use std::sync::Arc;

use rikka_theme::Token;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    Type,
    Function,
    Module,
    Constant,
    Field,
    Variable,
    Keyword,
    Word,
}

impl Kind {
    #[must_use]
    pub fn token(self) -> Option<Token> {
        Some(match self {
            Kind::Type => Token::Type,
            Kind::Function => Token::Function,
            Kind::Module => Token::Namespace,
            Kind::Constant => Token::Constant,
            Kind::Field => Token::Property,
            Kind::Variable => Token::Variable,
            Kind::Keyword => Token::Keyword,
            Kind::Word => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
    word: Arc<str>,
    kind: Kind,
    matched: Vec<Range<usize>>,
}

impl Suggestion {
    pub(crate) fn new(word: Arc<str>, kind: Kind, matched: Vec<Range<usize>>) -> Suggestion {
        Suggestion {
            word,
            kind,
            matched,
        }
    }

    #[must_use]
    pub fn word(&self) -> &str {
        &self.word
    }

    #[must_use]
    pub fn kind(&self) -> Kind {
        self.kind
    }

    #[must_use]
    pub fn matched(&self) -> &[Range<usize>] {
        &self.matched
    }
}

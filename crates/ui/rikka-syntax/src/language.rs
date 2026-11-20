use std::fmt;
use std::hash::{Hash, Hasher};
use std::path::Path;

use rikka_text::TextSpan;
use rikka_theme::Syntax;

use crate::comment::Comment;
use crate::grammar::{GRAMMARS, Grammar, Rules};
use crate::{regions_of, spans_of};
#[derive(Clone, Copy)]
pub struct Language(&'static Grammar);

impl Language {
    #[must_use]
    pub fn named(name: &str) -> Option<Language> {
        rikka_grammars::Grammar::named(name).and_then(Language::from_grammar)
    }

    #[must_use]
    pub fn from_path(path: impl AsRef<Path>) -> Option<Language> {
        rikka_grammars::Grammar::for_path(path).and_then(Language::from_grammar)
    }

    fn from_grammar(parsed: rikka_grammars::Grammar) -> Option<Language> {
        GRAMMARS
            .iter()
            .find(|grammar| grammar.name == parsed.name())
            .map(Language)
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        self.0.name
    }

    #[must_use]
    pub fn spans(self, text: &str, colors: Syntax) -> Vec<TextSpan> {
        spans_of(&regions_of(self, text), colors)
    }

    #[must_use]
    pub fn comment(self) -> Option<Comment> {
        self.0.comment
    }

    #[must_use]
    pub fn quotes(self) -> &'static str {
        self.0.quotes
    }

    #[must_use]
    pub fn openers(self) -> &'static str {
        self.0.openers
    }

    pub(crate) fn joins(self, letter: char) -> bool {
        self.0.joiners.contains(letter)
    }

    pub fn all() -> impl Iterator<Item = Language> {
        GRAMMARS.iter().map(Language)
    }

    pub(crate) fn rules(self) -> Option<&'static Rules> {
        self.0.rules()
    }
}
impl PartialEq for Language {
    fn eq(&self, other: &Language) -> bool {
        std::ptr::eq(self.0, other.0)
    }
}

impl Eq for Language {}

impl Hash for Language {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::ptr::from_ref(self.0).hash(state);
    }
}

impl fmt::Debug for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.name)
    }
}

use crate::language::Language;

pub const QUOTES: &str = "\"'";

#[must_use]
pub fn is_word(language: Option<Language>, letter: char) -> bool {
    letter.is_alphanumeric()
        || letter == '_'
        || language.is_some_and(|language| language.joins(letter))
}

pub(crate) fn is_word_shaped(language: Option<Language>, word: &str) -> bool {
    let mut letters = word.chars();
    letters
        .next()
        .is_some_and(|first| first.is_alphabetic() || first == '_' || first == '$')
        && letters.all(|letter| is_word(language, letter))
}

mod emoji;
mod parser;
mod rules;

use discord_core::Rich;

#[must_use]
pub fn parse(text: &str) -> Rich {
    parser::document(text)
}

#[must_use]
pub fn parse_line(text: &str) -> Rich {
    parser::line(text)
}

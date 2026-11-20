mod block;
mod css;
mod inline;
mod phrasing;
mod table;
mod text;
mod walk;

use crate::document::Document;

impl Document {
    #[must_use]
    pub fn to_markdown(&self) -> String {
        block::render(&walk::blocks(self))
    }

    #[must_use]
    pub fn to_text(&self) -> String {
        text::render(&walk::blocks(self))
    }
}

fn longest_backtick_run(text: &str) -> usize {
    text.split(|character| character != '`')
        .map(str::len)
        .max()
        .unwrap_or(0)
}

mod block;
mod css;
mod inline;
mod phrasing;
mod table;
mod text;
mod walk;

use crate::document::Document;
use walk::Pictures;

impl Document {
    #[must_use]
    pub fn to_markdown(&self) -> String {
        block::render(&walk::blocks(self, Pictures::Dropped))
    }

    #[must_use]
    pub fn to_markdown_with_pictures(
        &self,
        mut located: impl FnMut(&str) -> Option<String>,
    ) -> String {
        block::render(&block::locate(
            walk::blocks(self, Pictures::Kept),
            &mut located,
        ))
    }

    #[must_use]
    pub fn to_text(&self) -> String {
        text::render(&walk::blocks(self, Pictures::Dropped))
    }
}

fn longest_backtick_run(text: &str) -> usize {
    text.split(|character| character != '`')
        .map(str::len)
        .max()
        .unwrap_or(0)
}

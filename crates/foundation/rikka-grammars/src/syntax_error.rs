use std::ops::RangeInclusive;

use tree_sitter as ts;

use crate::grammar::Grammar;

#[derive(Debug)]
pub enum SyntaxError {
    Missing { line: usize, token: String },
    Unparsed { lines: RangeInclusive<usize> },
}

impl SyntaxError {
    #[must_use]
    pub fn line(&self) -> usize {
        match self {
            SyntaxError::Missing { line, .. } => *line,
            SyntaxError::Unparsed { lines } => *lines.start(),
        }
    }

    fn from_node(node: ts::Node<'_>) -> SyntaxError {
        let (start, end) = (node.start_position(), node.end_position());
        match node.is_missing() {
            true => SyntaxError::Missing {
                line: start.row + 1,
                token: node.kind().to_owned(),
            },
            false => SyntaxError::Unparsed {
                lines: start.row + 1
                    ..=end.row + usize::from(end.column > 0 || end.row == start.row),
            },
        }
    }
}

impl Grammar {
    #[must_use]
    pub fn syntax_errors(self, text: &str) -> Option<Vec<SyntaxError>> {
        let tree = self.parse(text)?;
        let mut found = Vec::new();
        let mut waiting = vec![tree.root_node()];
        while let Some(node) = waiting.pop() {
            if node.is_missing() || node.is_error() {
                found.push(SyntaxError::from_node(node));
                continue;
            }
            if node.has_error() {
                waiting.extend(node.children(&mut node.walk()));
            }
        }
        found.sort_by_key(SyntaxError::line);
        Some(found)
    }

    pub(crate) fn parse(self, text: &str) -> Option<ts::Tree> {
        let mut parser = ts::Parser::new();
        parser.set_language(self.language()?).ok()?;
        parser.parse(text, None)
    }
}

use std::fmt::Write;
use std::path::Path;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Span {
    pub(crate) line: usize,
    pub(crate) column: usize,
    pub(crate) length: usize,
}

impl Span {
    pub(crate) fn new(line: usize, column: usize, length: usize) -> Self {
        Self {
            line,
            column,
            length,
        }
    }

    pub(crate) fn shifted(self, columns: usize, length: usize) -> Self {
        Self {
            line: self.line,
            column: self.column + columns,
            length,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Diagnostic {
    pub(crate) span: Span,
    pub(crate) message: String,
}

impl Diagnostic {
    pub(crate) fn new(span: Span, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
        }
    }

    pub(crate) fn render(&self, path: &Path, source: &str) -> String {
        let Span {
            line,
            column,
            length,
        } = self.span;
        let mut rendered = format!(
            "{}:{line}:{column}: error: {}",
            path.display(),
            self.message
        );
        let Some(text) = source.lines().nth(line.saturating_sub(1)) else {
            return rendered;
        };
        let gutter = line.to_string().len();
        let indent: String = text
            .chars()
            .take(column.saturating_sub(1))
            .map(|character| match character {
                '\t' => '\t',
                _ => ' ',
            })
            .collect();
        let carets = "^".repeat(length.max(1));
        write!(
            rendered,
            "\n{:gutter$} |\n{line} | {text}\n{:gutter$} | {indent}{carets}",
            "", ""
        )
        .expect("writing to a String cannot fail");
        rendered
    }
}

pub(crate) type Result<T, E = Diagnostic> = std::result::Result<T, E>;

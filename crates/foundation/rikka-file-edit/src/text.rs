use std::borrow::Cow;

use crate::eol::Eol;

const BOM: &str = "\u{feff}";

pub(crate) fn cut_line(line: &str, most: usize) -> Cow<'_, str> {
    match line.char_indices().nth(most) {
        Some((at, _)) => Cow::Owned(format!(
            "{}… [{} more characters]",
            &line[..at],
            line[at..].chars().count()
        )),
        None => Cow::Borrowed(line),
    }
}

#[must_use]
pub(crate) fn lines_of(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return Vec::new();
    }
    split(text.strip_suffix('\n').unwrap_or(text))
}

fn split(text: &str) -> Vec<&str> {
    text.split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect()
}

#[derive(Clone, Copy, Debug, thiserror::Error)]
#[error("content is not UTF-8 text")]
pub struct NotText;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Text {
    content: String,
    starts: Vec<usize>,
    eol: Eol,
    bom: bool,
}

impl Text {
    pub fn parse(bytes: &[u8]) -> Result<Text, NotText> {
        let text = std::str::from_utf8(bytes).map_err(|_| NotText)?;
        match bytes.contains(&0) {
            true => Err(NotText),
            false => Ok(Text::read(text)),
        }
    }

    #[must_use]
    pub fn from_content(content: &str) -> Text {
        Text::new(normalize_lines(content, true), Eol::Lf, false)
    }

    #[must_use]
    pub fn with_content(&self, content: &str) -> Text {
        Text::new(
            normalize_lines(content, self.ends_with_newline()),
            self.eol,
            self.bom,
        )
    }

    #[must_use]
    pub fn with_exact_content(&self, content: &str) -> Text {
        Text::new(content.to_owned(), self.eol, self.bom)
    }

    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }

    #[must_use]
    pub fn line_count(&self) -> usize {
        self.starts.len()
    }

    #[must_use]
    pub fn line(&self, number: usize) -> Option<&str> {
        let (start, end) = self.bounds(number)?;
        Some(&self.content[start..end])
    }

    pub fn lines(&self) -> impl Iterator<Item = &str> {
        (1..=self.line_count()).filter_map(|number| self.line(number))
    }

    #[must_use]
    pub fn render(&self) -> Vec<u8> {
        let mut written = String::with_capacity(self.content.len() + BOM.len());
        if self.bom && !self.content.is_empty() {
            written.push_str(BOM);
        }
        match self.eol {
            Eol::Lf => written.push_str(&self.content),
            Eol::Crlf => written.push_str(&self.content.replace('\n', "\r\n")),
        }
        written.into_bytes()
    }

    fn read(text: &str) -> Text {
        let bom = text.starts_with(BOM);
        let body = match bom {
            true => &text[BOM.len()..],
            false => text,
        };
        let eol = Eol::detect(body);
        let content = match eol {
            Eol::Lf => body.to_owned(),
            Eol::Crlf => body.replace("\r\n", "\n"),
        };
        Text::new(content, eol, bom)
    }

    fn new(content: String, eol: Eol, bom: bool) -> Text {
        Text {
            starts: starts(&content),
            content,
            eol,
            bom,
        }
    }

    pub(crate) fn ends_with_newline(&self) -> bool {
        self.content.is_empty() || self.content.ends_with('\n')
    }

    fn bounds(&self, number: usize) -> Option<(usize, usize)> {
        let start = *self.starts.get(number.checked_sub(1)?)?;
        let end = match self.starts.get(number) {
            Some(&next) => next - 1,
            None => self.content.len() - usize::from(self.ends_with_newline()),
        };
        Some((start, end))
    }
}

fn starts(content: &str) -> Vec<usize> {
    if content.is_empty() {
        return Vec::new();
    }

    let mut starts = vec![0];
    starts.extend(
        content
            .match_indices('\n')
            .map(|(at, _)| at + 1)
            .filter(|&start| start < content.len()),
    );
    starts
}

fn normalize_lines(content: &str, newline: bool) -> String {
    let lines = lines_of(content);
    let mut body = lines.join("\n");
    if newline && !body.is_empty() {
        body.push('\n');
    }
    body
}

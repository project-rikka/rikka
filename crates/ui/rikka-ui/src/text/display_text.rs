use std::ops::Range;

use rikka_text::Affinity;
use rikka_text_document::Content;

pub const BULLET: char = '\u{2022}';

const BULLET_LEN: usize = BULLET.len_utf8();

#[derive(Clone, Copy, Debug)]
pub struct DisplayText<'a> {
    content: &'a Content,
    secret: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplayRevision {
    text: u64,
    composing: u64,
    secret: bool,
}

impl<'a> DisplayText<'a> {
    pub fn new(content: &'a Content) -> Self {
        Self {
            content,
            secret: false,
        }
    }

    pub fn secret(content: &'a Content) -> Self {
        Self {
            content,
            secret: true,
        }
    }

    pub fn is_secret(&self) -> bool {
        self.secret
    }

    pub fn content(&self) -> &'a Content {
        self.content
    }

    pub fn revision(&self) -> DisplayRevision {
        DisplayRevision {
            text: self.content.revision(),
            composing: self.content.preedit_revision(),
            secret: self.secret,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.content.is_empty() && self.inserted().is_empty()
    }

    pub fn line_count(&self) -> usize {
        self.content.line_count()
    }

    pub fn composing(&self) -> &str {
        match self.secret {
            true => "",
            false => self.content.preedit(),
        }
    }

    pub fn ghost(&self) -> Option<Range<usize>> {
        if self.secret || !self.content.preedit().is_empty() {
            return None;
        }
        let ghost = self.content.ghost();
        let caret = self.content.caret();
        (!ghost.is_empty()).then(|| caret..caret + ghost.len())
    }

    pub fn composing_line(&self) -> Option<usize> {
        (!self.inserted().is_empty()).then(|| self.content.line_of(self.content.caret()))
    }

    fn inserted(&self) -> &'a str {
        match self.secret {
            true => "",
            false if self.content.preedit().is_empty() => self.content.ghost(),
            false => self.content.preedit(),
        }
    }

    pub fn offset(&self, at: usize) -> usize {
        self.shifted(at, false)
    }

    pub fn range(&self, range: Range<usize>) -> Range<usize> {
        self.shifted(range.start, false)..self.shifted(range.end, true)
    }

    pub fn document_offset(&self, at: usize) -> usize {
        if self.secret {
            let rope = self.content.rope();
            return rope.char_to_byte(at / BULLET_LEN);
        }
        let caret = self.content.caret();
        let inserted = self.inserted().len();
        match at {
            _ if at <= caret => at,
            _ if at <= caret + inserted => caret,
            _ => at - inserted,
        }
    }

    pub fn caret(&self) -> usize {
        if self.secret {
            return self.offset(self.content.caret());
        }
        let within = match self.content.preedit_cursor() {
            Some((start, _)) if !self.composing().is_empty() => start,
            _ => self.composing().len(),
        };
        self.content.caret() + within
    }

    pub fn affinity(&self) -> Affinity {
        match self.composing().is_empty() {
            true => self.content.affinity(),
            false => Affinity::After,
        }
    }

    pub fn preedit(&self) -> Option<Range<usize>> {
        let composing = self.composing();
        let caret = self.content.caret();
        (!composing.is_empty()).then(|| caret..caret + composing.len())
    }

    pub fn converting(&self) -> Option<Range<usize>> {
        let (start, end) = self
            .content
            .preedit_cursor()
            .filter(|_| !self.composing().is_empty())?;
        let caret = self.content.caret();
        (start != end).then(|| caret + start..caret + end)
    }

    pub fn line_of(&self, at: usize) -> usize {
        self.content.line_of(self.document_offset(at))
    }

    pub fn line_start(&self, line: usize) -> usize {
        self.offset(self.content.line_start(line))
    }

    pub fn line_range(&self, line: usize) -> Range<usize> {
        self.range(self.content.line_range(line))
    }

    pub fn line_into(&self, line: usize, out: &mut String) {
        if self.secret {
            let range = self.content.line_range(line);
            out.extend(std::iter::repeat_n(BULLET, self.characters(range)));
            return;
        }
        let at = out.len();
        self.content.line_into(line, out);
        if self.composing_line() != Some(line) {
            return;
        }
        let within = self.content.caret() - self.content.line_start(line);
        out.insert_str(at + within, self.inserted());
    }

    pub fn slice(&self, range: Range<usize>) -> String {
        if self.secret {
            let count = range.end / BULLET_LEN - range.start / BULLET_LEN;
            return std::iter::repeat_n(BULLET, count).collect();
        }
        let composing = self.inserted();
        if composing.is_empty() {
            return self.content.slice(range);
        }
        let caret = self.content.caret();
        let composed = caret..caret + composing.len();
        let within = |at: usize| at.clamp(composed.start, composed.end) - composed.start;
        let mut out = self
            .content
            .slice(range.start.min(composed.start)..range.end.min(composed.start));
        out.push_str(&composing[within(range.start)..within(range.end)]);
        out.push_str(&self.content.slice(
            range.start.max(composed.end) - composing.len()
                ..range.end.max(composed.end) - composing.len(),
        ));
        out
    }

    pub fn head(&self, lines: usize) -> String {
        let last = lines.clamp(1, self.line_count()) - 1;
        self.slice(0..self.line_range(last).end)
    }

    fn shifted(&self, at: usize, trailing: bool) -> usize {
        if self.secret {
            let rope = self.content.rope();
            return rope.byte_to_char(at) * BULLET_LEN;
        }
        let caret = self.content.caret();
        let past = match trailing {
            true => at >= caret,
            false => at > caret,
        };
        match past {
            true => at + self.inserted().len(),
            false => at,
        }
    }

    fn characters(&self, range: Range<usize>) -> usize {
        let rope = self.content.rope();
        rope.byte_to_char(range.end) - rope.byte_to_char(range.start)
    }
}

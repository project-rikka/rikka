mod chunks;
mod node;

use std::borrow::Cow;
use std::fmt;
use std::ops::Range;
use std::sync::Arc;

use rikka_text_layout::TextSource;

pub use chunks::Chunks;
use chunks::ChunksBefore;
use node::{CHUNK_FILL, Link, Node, after_break, breaks_in, extent, grow, merge, shrink, split};

const SPARSE_CHUNKS: usize = 32;

const SPARSE_FILL: usize = CHUNK_FILL / 8;

#[derive(Clone, Default)]
pub struct Rope {
    root: Link,
}

impl Rope {
    #[must_use]
    pub fn new() -> Rope {
        Rope::default()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        extent(&self.root).bytes
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    #[must_use]
    pub fn char_count(&self) -> usize {
        extent(&self.root).chars
    }

    #[must_use]
    pub fn line_count(&self) -> usize {
        extent(&self.root).breaks + 1
    }

    #[must_use]
    pub fn line_of(&self, at: usize) -> usize {
        let mut at = at.min(self.len());
        let mut line = 0;
        let mut below = self.root.as_deref();
        while let Some(node) = below {
            let left = extent(&node.left);
            let end = left.bytes + node.chunk.len();
            if at < left.bytes {
                below = node.left.as_deref();
            } else if at <= end {
                let head = &node.chunk.as_bytes()[..at - left.bytes];
                return line + left.breaks + breaks_in(head);
            } else {
                line += left.breaks + node.own().breaks;
                at -= end;
                below = node.right.as_deref();
            }
        }
        line
    }

    #[must_use]
    pub fn line_start(&self, line: usize) -> usize {
        let mut breaks = line;
        let mut passed = 0;
        let mut below = self.root.as_deref();
        while let Some(node) = below {
            let left = extent(&node.left);
            let own = node.own().breaks;
            if breaks <= left.breaks {
                below = node.left.as_deref();
            } else if breaks <= left.breaks + own {
                let within = after_break(node.chunk.as_bytes(), breaks - left.breaks - 1);
                return passed + left.bytes + within;
            } else {
                breaks -= left.breaks + own;
                passed += left.bytes + node.chunk.len();
                below = node.right.as_deref();
            }
        }
        match line {
            0 => 0,
            _ => self.len(),
        }
    }

    #[must_use]
    pub fn line_span(&self, line: usize) -> Range<usize> {
        let start = self.line_start(line);
        let end = match line + 1 < self.line_count() {
            true => self.line_start(line + 1) - 1,
            false => self.len(),
        };
        start..end
    }

    #[must_use]
    pub fn byte_to_char(&self, at: usize) -> usize {
        let mut at = at.min(self.len());
        let mut chars = 0;
        let mut below = self.root.as_deref();
        while let Some(node) = below {
            let left = extent(&node.left);
            let end = left.bytes + node.chunk.len();
            if at < left.bytes {
                below = node.left.as_deref();
            } else if at <= end {
                let head = node.chunk.floor_char_boundary(at - left.bytes);
                return chars + left.chars + node.chunk[..head].chars().count();
            } else {
                chars += left.chars + node.own().chars;
                at -= end;
                below = node.right.as_deref();
            }
        }
        chars
    }

    #[must_use]
    pub fn char_to_byte(&self, index: usize) -> usize {
        let mut index = index.min(self.char_count());
        let mut passed = 0;
        let mut below = self.root.as_deref();
        while let Some(node) = below {
            let left = extent(&node.left);
            let own = node.own().chars;
            if index < left.chars {
                below = node.left.as_deref();
            } else if index <= left.chars + own {
                let within = node
                    .chunk
                    .char_indices()
                    .nth(index - left.chars)
                    .map_or(node.chunk.len(), |(at, _)| at);
                return passed + left.bytes + within;
            } else {
                index -= left.chars + own;
                passed += left.bytes + node.chunk.len();
                below = node.right.as_deref();
            }
        }
        passed
    }

    #[must_use]
    pub fn floor_char_boundary(&self, at: usize) -> usize {
        let (chunk, start) = self.chunk_at(at);
        start + chunk.floor_char_boundary(at.saturating_sub(start))
    }

    #[must_use]
    pub fn ceil_char_boundary(&self, at: usize) -> usize {
        let (chunk, start) = self.chunk_at(at);
        start + chunk.ceil_char_boundary(at.saturating_sub(start))
    }

    #[must_use]
    pub fn chunk_at(&self, at: usize) -> (&str, usize) {
        match self.holder(at) {
            Some((node, start)) => (&node.chunk, start),
            None => ("", self.len()),
        }
    }

    #[must_use]
    pub fn char_at(&self, at: usize) -> Option<char> {
        self.chars_from(at).next()
    }

    #[must_use]
    pub fn char_before(&self, at: usize) -> Option<char> {
        self.chars_before(at).next()
    }

    #[must_use]
    pub fn chunks(&self, range: Range<usize>) -> Chunks<'_> {
        Chunks::within(&self.root, range)
    }

    pub fn chars_from(&self, at: usize) -> impl Iterator<Item = char> + '_ {
        self.chunks(at.min(self.len())..self.len())
            .flat_map(str::chars)
    }

    pub fn chars_before(&self, at: usize) -> impl Iterator<Item = char> + '_ {
        ChunksBefore::before(&self.root, at.min(self.len())).flat_map(|chunk| chunk.chars().rev())
    }

    #[must_use]
    pub fn slice(&self, range: Range<usize>) -> String {
        let mut text = String::with_capacity(range.len());
        text.extend(self.chunks(range));
        text
    }

    pub fn insert(&mut self, at: usize, text: &str) {
        if text.is_empty() || grow(&mut self.root, at, text) {
            return;
        }
        let (head, tail) = split(self.root.take(), at);
        self.root = merge(merge(head, node::planted(text)), tail);
        self.settle();
    }

    pub fn remove(&mut self, range: Range<usize>) {
        if range.is_empty() || shrink(&mut self.root, range.clone()) {
            return;
        }
        let (rest, tail) = split(self.root.take(), range.end);
        let (head, _) = split(rest, range.start);
        self.root = merge(head, tail);
        self.settle();
    }

    fn settle(&mut self) {
        let whole = extent(&self.root);
        if whole.chunks >= SPARSE_CHUNKS && whole.bytes < whole.chunks * SPARSE_FILL {
            *self = Rope::from(self.to_string().as_str());
        }
    }

    fn holder(&self, at: usize) -> Option<(&Node, usize)> {
        let mut at = at;
        let mut passed = 0;
        let mut below = self.root.as_deref();
        while let Some(node) = below {
            let before = extent(&node.left).bytes;
            let end = before + node.chunk.len();
            if at < before {
                below = node.left.as_deref();
            } else if at < end {
                return Some((node, passed + before));
            } else {
                at -= end;
                passed += end;
                below = node.right.as_deref();
            }
        }
        None
    }

    fn bytes(&self) -> impl Iterator<Item = u8> + '_ {
        self.chunks(0..self.len()).flat_map(str::bytes)
    }
}

impl From<&str> for Rope {
    fn from(text: &str) -> Rope {
        Rope {
            root: node::planted(text),
        }
    }
}

impl fmt::Display for Rope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.chunks(0..self.len())
            .try_for_each(|chunk| f.write_str(chunk))
    }
}

impl fmt::Debug for Rope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Rope")
            .field("bytes", &self.len())
            .field("lines", &self.line_count())
            .finish()
    }
}

impl PartialEq for Rope {
    fn eq(&self, other: &Rope) -> bool {
        let shared = match (&self.root, &other.root) {
            (Some(one), Some(other)) => Arc::ptr_eq(one, other),
            (None, None) => true,
            _ => false,
        };
        shared || (self.len() == other.len() && self.bytes().eq(other.bytes()))
    }
}

impl Eq for Rope {}

impl PartialEq<str> for Rope {
    fn eq(&self, other: &str) -> bool {
        self.len() == other.len() && self.bytes().eq(other.bytes())
    }
}

impl PartialEq<&str> for Rope {
    fn eq(&self, other: &&str) -> bool {
        self == *other
    }
}

impl TextSource for Rope {
    fn byte_len(&self) -> usize {
        self.len()
    }

    fn covering(&self, range: Range<usize>) -> (usize, Cow<'_, str>) {
        let start = self.floor_char_boundary(range.start);
        let end = self.ceil_char_boundary(range.end);
        let (chunk, at) = self.chunk_at(start);
        match end <= at + chunk.len() {
            true => (start, Cow::Borrowed(&chunk[start - at..end - at])),
            false => (start, Cow::Owned(self.slice(start..end))),
        }
    }
}

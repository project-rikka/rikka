use std::ops::Range;

use crate::rope::node::{Link, Node, extent};

#[derive(Clone, Debug)]
pub struct Chunks<'a> {
    pending: Vec<&'a Node>,
    skipped: usize,
    remaining: usize,
}

impl<'a> Chunks<'a> {
    pub(crate) fn within(root: &'a Link, range: Range<usize>) -> Chunks<'a> {
        let mut pending = Vec::new();
        let mut skipped = 0;
        let mut at = range.start;
        let mut below = root.as_deref();
        while let Some(node) = below {
            let before = extent(&node.left).bytes;
            let end = before + node.chunk.len();
            if at < before {
                pending.push(node);
                below = node.left.as_deref();
            } else if at < end {
                pending.push(node);
                skipped = at - before;
                break;
            } else {
                at -= end;
                below = node.right.as_deref();
            }
        }
        Chunks {
            pending,
            skipped,
            remaining: range.len(),
        }
    }
}

impl<'a> Iterator for Chunks<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        if self.remaining == 0 {
            return None;
        }
        let node = self.pending.pop()?;
        let mut below = node.right.as_deref();
        while let Some(lower) = below {
            self.pending.push(lower);
            below = lower.left.as_deref();
        }
        let rest = &node.chunk[std::mem::take(&mut self.skipped)..];
        let taken = &rest[..rest.len().min(self.remaining)];
        self.remaining -= taken.len();
        Some(taken)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ChunksBefore<'a> {
    pending: Vec<&'a Node>,
    kept: Option<usize>,
}

impl<'a> ChunksBefore<'a> {
    pub(crate) fn before(root: &'a Link, at: usize) -> ChunksBefore<'a> {
        let mut pending = Vec::new();
        let mut kept = None;
        let mut at = at;
        let mut below = root.as_deref();
        while let Some(node) = below {
            let before = extent(&node.left).bytes;
            let end = before + node.chunk.len();
            if at > end {
                pending.push(node);
                at -= end;
                below = node.right.as_deref();
            } else if at > before {
                pending.push(node);
                kept = Some(at - before);
                break;
            } else {
                below = node.left.as_deref();
            }
        }
        ChunksBefore { pending, kept }
    }
}

impl<'a> Iterator for ChunksBefore<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        let node = self.pending.pop()?;
        let mut below = node.left.as_deref();
        while let Some(lower) = below {
            self.pending.push(lower);
            below = lower.right.as_deref();
        }
        Some(match self.kept.take() {
            Some(kept) => &node.chunk[..kept],
            None => &node.chunk,
        })
    }
}

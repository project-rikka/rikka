mod haystack;
mod measure;
mod needle;
mod ranking;
mod score;

pub use haystack::Haystack;
pub use needle::Needle;
pub use ranking::{best, ranked};
pub use score::Score;

use std::ops::Range;

#[must_use]
pub fn byte_ranges(text: &str, positions: &[usize]) -> Vec<Range<usize>> {
    let mut ranges: Vec<Range<usize>> = Vec::new();
    let mut matched = positions.iter().copied().peekable();
    for (at, (offset, letter)) in text.char_indices().enumerate() {
        if matched.peek() != Some(&at) {
            continue;
        }
        matched.next();
        let end = offset + letter.len_utf8();
        match ranges.last_mut() {
            Some(last) if last.end == offset => last.end = end,
            _ => ranges.push(offset..end),
        }
    }
    ranges
}

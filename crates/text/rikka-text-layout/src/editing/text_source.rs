use std::borrow::Cow;
use std::ops::Range;

pub trait TextSource {
    fn byte_len(&self) -> usize;

    fn covering(&self, range: Range<usize>) -> (usize, Cow<'_, str>);
}

impl TextSource for str {
    fn byte_len(&self) -> usize {
        self.len()
    }

    fn covering(&self, range: Range<usize>) -> (usize, Cow<'_, str>) {
        let start = self.floor_char_boundary(range.start);
        let end = self.ceil_char_boundary(range.end);
        (start, Cow::Borrowed(&self[start..end]))
    }
}

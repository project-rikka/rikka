use std::ops::Range;

use rikka_text_document::{Chunks, Place, Rope, Splice, place_of};
use tree_sitter as ts;

type Bytes<'a> = std::iter::Map<Chunks<'a>, fn(&'a str) -> &'a [u8]>;

#[derive(Clone, Copy)]
pub(crate) struct RopeProvider<'a>(pub(crate) &'a Rope);

impl<'a> ts::TextProvider<&'a [u8]> for RopeProvider<'a> {
    type I = Bytes<'a>;

    fn text(&mut self, node: ts::Node<'_>) -> Bytes<'a> {
        self.0
            .chunks(node.byte_range())
            .map(str::as_bytes as fn(&'a str) -> &'a [u8])
    }
}

pub(crate) fn chunk_at(rope: &Rope, byte: usize) -> &str {
    let (chunk, start) = rope.chunk_at(byte);
    &chunk[byte.min(rope.len()) - start..]
}

fn point_of(place: Place) -> ts::Point {
    ts::Point::new(place.row, place.column)
}

pub(crate) fn edit_of(splice: &Splice) -> ts::InputEdit {
    ts::InputEdit {
        start_byte: splice.start,
        old_end_byte: splice.old_end,
        new_end_byte: splice.new_end,
        start_position: point_of(splice.start_place),
        old_end_position: point_of(splice.old_end_place),
        new_end_position: point_of(splice.new_end_place),
    }
}

pub(crate) fn span_of(rope: &Rope, range: &Range<usize>) -> ts::Range {
    ts::Range {
        start_byte: range.start,
        end_byte: range.end,
        start_point: point_of(place_of(rope, range.start)),
        end_point: point_of(place_of(rope, range.end)),
    }
}

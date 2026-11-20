use std::cell::RefCell;

use rikka_text_unicode::{LineBreakClass, graphemes, line_break_class};

use crate::layout::{ParagraphLayout, Row};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GraphemeBox {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) left: f32,
    pub(crate) right: f32,
    pub(crate) rtl: bool,
}

impl GraphemeBox {
    #[inline]
    pub(crate) fn is_mark(&self) -> bool {
        self.start == self.end
    }

    #[inline]
    pub(crate) fn middle(&self) -> f32 {
        (self.left + self.right) / 2.0
    }
}

thread_local! {
    static BOXES: RefCell<Vec<GraphemeBox>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn with_boxes<T>(visit: impl FnOnce(&mut Vec<GraphemeBox>) -> T) -> T {
    BOXES.with(|boxes| visit(&mut boxes.borrow_mut()))
}

pub(crate) fn row_boxes(layout: &ParagraphLayout, row: &Row, boxes: &mut Vec<GraphemeBox>) {
    boxes.clear();
    let text = layout.text();
    let glyphs = layout.glyphs(row);
    let mut pen = row.origin;
    let mut index = 0;
    while index < glyphs.len() {
        let first = &glyphs[index];
        let left = pen;
        let mut next = index;
        while next < glyphs.len()
            && glyphs[next].start == first.start
            && glyphs[next].end == first.end
        {
            pen += glyphs[next].advance;
            next += 1;
        }
        index = next;
        let (start, end, rtl) = (first.start as usize, first.end as usize, first.is_rtl());
        if start == end {
            boxes.push(GraphemeBox {
                start,
                end,
                left,
                right: pen,
                rtl,
            });
            continue;
        }
        let cluster = &text[start..end];
        if pen == left && is_line_break(cluster) {
            continue;
        }
        let count = graphemes(cluster).count().max(1);
        if count == 1 {
            boxes.push(GraphemeBox {
                start,
                end,
                left,
                right: pen,
                rtl,
            });
            continue;
        }
        let share = (pen - left) / count as f32;
        let first = boxes.len();
        for (position, (offset, grapheme)) in graphemes(cluster).enumerate() {
            let slot = if rtl { count - 1 - position } else { position };
            boxes.push(GraphemeBox {
                start: start + offset,
                end: start + offset + grapheme.len(),
                left: left + share * slot as f32,
                right: left + share * (slot + 1) as f32,
                rtl,
            });
        }
        if rtl {
            boxes[first..].reverse();
        }
    }
}

fn is_line_break(cluster: &str) -> bool {
    cluster.chars().next().is_some_and(|character| {
        matches!(
            line_break_class(character),
            LineBreakClass::MandatoryBreak
                | LineBreakClass::CarriageReturn
                | LineBreakClass::LineFeed
                | LineBreakClass::NextLine
        )
    })
}

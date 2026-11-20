use rikka_text_layout::ParagraphLayout;

use crate::layout_row::LayoutRow;

#[derive(Clone, Copy, Debug)]
pub(crate) struct LineStart {
    pub(crate) row: u32,
    pub(crate) text: usize,
}

pub(crate) struct Rows<'a> {
    paragraphs: &'a [ParagraphLayout],
    starts: &'a [LineStart],
    line_height: f32,
    line: usize,
    within: usize,
    above: Option<f32>,
    below: f32,
}

impl<'a> Rows<'a> {
    pub(crate) fn new(
        paragraphs: &'a [ParagraphLayout],
        starts: &'a [LineStart],
        line_height: f32,
        row: usize,
    ) -> Self {
        let line = starts
            .partition_point(|start| start.row as usize <= row)
            .saturating_sub(1);
        let first = starts.get(line).map_or(0, |start| start.row as usize);
        Rows {
            paragraphs,
            starts,
            line_height,
            line,
            within: row - first,
            above: None,
            below: f32::INFINITY,
        }
    }

    pub(crate) fn between(mut self, top: f32, bottom: f32) -> Self {
        self.above = Some(top);
        self.below = bottom;
        self
    }
}

impl<'a> Iterator for Rows<'a> {
    type Item = LayoutRow<'a>;

    fn next(&mut self) -> Option<LayoutRow<'a>> {
        loop {
            let layout = self.paragraphs.get(self.line)?;
            let Some(row) = layout.rows().get(self.within) else {
                self.line += 1;
                self.within = 0;
                continue;
            };
            let start = self.starts[self.line];
            let index = start.row as usize + self.within;
            let shown = LayoutRow {
                index,
                line: self.line,
                line_start: start.text,
                top: index as f32 * self.line_height,
                layout,
                row,
            };
            self.within += 1;
            if let Some(top) = self.above {
                if shown.bottom() < top {
                    continue;
                }
                self.above = None;
            }
            if shown.top() >= self.below {
                self.line = self.paragraphs.len();
                return None;
            }
            return Some(shown);
        }
    }
}

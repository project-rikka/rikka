use rikka_text_unicode::{BreakOpportunity, LineBreakClass, line_break_class, line_breaks};

use crate::shaping::columns::{GlyphColumns, flag};
use crate::shaping::measure::Measure;
use crate::shaping::run::{Run, RunStyle};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Segment {
    pub(crate) start: u32,
    pub(crate) end: u32,
    pub(crate) content_end: u32,
    pub(crate) glyphs_start: u32,
    pub(crate) glyphs_end: u32,
    pub(crate) content: Measure,
    pub(crate) trailing: Measure,
    pub(crate) content_separators: u32,
    pub(crate) trailing_separators: u32,
    pub(crate) mandatory: bool,
    pub(crate) tab: bool,
}

impl Segment {
    #[inline]
    pub(crate) fn has_content(&self) -> bool {
        self.content_end > self.start
    }
}

pub(crate) fn segments(
    text: &str,
    runs: &[Run],
    styles: &[RunStyle],
    glyphs: &GlyphColumns,
    out: &mut Vec<Segment>,
) {
    out.clear();
    let ends_with_break = text.chars().next_back().is_some_and(|last| {
        matches!(
            line_break_class(last),
            LineBreakClass::MandatoryBreak
                | LineBreakClass::CarriageReturn
                | LineBreakClass::LineFeed
                | LineBreakClass::NextLine
        )
    });
    let mut builder = Builder {
        runs,
        styles,
        glyphs,
        glyph: 0,
        run: 0,
        segment: Segment::default(),
    };
    for opportunity in line_breaks(text) {
        let offset = opportunity.offset as u32;
        let mut at = builder.glyph;
        while at < glyphs.len() && glyphs.clusters[at] < offset {
            at += 1;
        }
        let inside_cluster = at > 0 && glyphs.cluster_ends[at - 1] > offset;
        if inside_cluster && offset < text.len() as u32 {
            continue;
        }
        builder.take_until(at);
        let mandatory = opportunity.opportunity == BreakOpportunity::Mandatory
            && (opportunity.offset < text.len() || ends_with_break);
        out.push(builder.finish(offset, mandatory));
    }
}

struct Builder<'a> {
    runs: &'a [Run],
    styles: &'a [RunStyle],
    glyphs: &'a GlyphColumns,
    glyph: usize,
    run: usize,
    segment: Segment,
}

impl Builder<'_> {
    fn take_until(&mut self, end: usize) {
        let glyphs = self.glyphs;
        while self.glyph < end {
            while self.runs[self.run].glyphs.end as usize <= self.glyph {
                self.run += 1;
            }
            let run = &self.runs[self.run];
            let size = self.styles[usize::from(run.style)].size;
            let stop = end.min(run.glyphs.end as usize);
            for index in self.glyph..stop {
                let flags = glyphs.flags[index];
                let advance = glyphs.advances[index];
                let segment = &mut self.segment;
                segment.tab |= flags & flag::TAB != 0;
                let separator = u32::from(flags & flag::WORD_SEPARATOR != 0);
                let ends_cluster = glyphs.ends_spaced_cluster(index);
                if flags & flag::WHITESPACE != 0 {
                    segment.trailing.add(advance, size, ends_cluster);
                    segment.trailing_separators += separator;
                } else {
                    segment.content.append(segment.trailing);
                    segment.content.add(advance, size, ends_cluster);
                    segment.content_separators += segment.trailing_separators;
                    segment.trailing = Measure::default();
                    segment.trailing_separators = 0;
                    segment.content_end = glyphs.cluster_ends[index];
                }
            }
            self.glyph = stop;
        }
    }

    fn finish(&mut self, end: u32, mandatory: bool) -> Segment {
        let segment = Segment {
            end,
            glyphs_end: self.glyph as u32,
            mandatory,
            ..self.segment
        };
        self.segment = Segment {
            start: end,
            end,
            content_end: end,
            glyphs_start: self.glyph as u32,
            glyphs_end: self.glyph as u32,
            ..Segment::default()
        };
        segment
    }
}

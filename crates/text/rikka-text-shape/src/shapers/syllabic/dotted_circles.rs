use crate::buffer::glyph_buffer::Buffer;
use crate::buffer::glyph_info::GlyphInfo;
use crate::buffer::scratch_flags;
use crate::common::buffer_flags::BufferFlags;
use crate::face::font_context::FontContext;
use crate::shapers::syllabic::syllables::syllable_kind;

const DOTTED_CIRCLE: u32 = 0x25CC;

#[derive(Clone, Copy, Debug)]
pub(crate) struct DottedCircle {
    pub(crate) broken: u8,
    pub(crate) category: u8,
    pub(crate) position: u8,
    pub(crate) after_repha: Option<u8>,
}

impl DottedCircle {
    pub(crate) fn insert(&self, font: &FontContext, buffer: &mut Buffer) -> bool {
        if buffer
            .flags
            .contains(BufferFlags::DO_NOT_INSERT_DOTTED_CIRCLE)
            || buffer.scratch_flags & scratch_flags::HAS_BROKEN_SYLLABLE == 0
        {
            return false;
        }
        let Some(glyph) = font.nominal_glyph(DOTTED_CIRCLE) else {
            return false;
        };
        let template = GlyphInfo {
            codepoint: u32::from(glyph),
            shaper_category: self.category,
            shaper_auxiliary: self.position,
            ..GlyphInfo::default()
        };
        buffer.clear_output();
        buffer.idx = 0;
        let mut last_syllable = 0;
        while buffer.idx < buffer.len && buffer.successful {
            let current = *buffer.cur(0);
            if last_syllable == current.syllable || syllable_kind(&current) != self.broken {
                buffer.next_glyph();
                continue;
            }
            last_syllable = current.syllable;
            let mut circle = template;
            circle.cluster = current.cluster;
            circle.mask = current.mask;
            circle.syllable = current.syllable;
            if let Some(repha) = self.after_repha {
                while buffer.idx < buffer.len
                    && buffer.cur(0).syllable == last_syllable
                    && buffer.cur(0).shaper_category == repha
                {
                    buffer.next_glyph();
                }
            }
            buffer.output_info(circle);
        }
        buffer.sync();
        true
    }
}

use rikka_text_font::Tag;

use crate::buffer::glyph_buffer::Buffer;
use crate::pipeline::normalize::NormalizeContext;
use crate::plan::shape_plan::PlanCore;
use crate::shapers::complex_shaper::ComplexShaper;
use crate::unicode::combining_class::{
    BELOW, HEBREW_HIRIQ, HEBREW_METEG, HEBREW_PATAH, HEBREW_QAMATS, HEBREW_SHEVA,
};
use crate::unicode::to_char;

#[derive(Debug)]
pub(crate) struct HebrewShaper;

pub(crate) static HEBREW_SHAPER: HebrewShaper = HebrewShaper;

const DAGESH_FORMS: [u32; 27] = [
    0xFB30, 0xFB31, 0xFB32, 0xFB33, 0xFB34, 0xFB35, 0xFB36, 0x0000, 0xFB38, 0xFB39, 0xFB3A, 0xFB3B,
    0xFB3C, 0x0000, 0xFB3E, 0x0000, 0xFB40, 0xFB41, 0x0000, 0xFB43, 0xFB44, 0x0000, 0xFB46, 0xFB47,
    0xFB48, 0xFB49, 0xFB4A,
];

fn presentation_form(base: u32, mark: u32) -> Option<u32> {
    let composed = match (mark, base) {
        (0x05B4, 0x05D9) => 0xFB1D,
        (0x05B7, 0x05D9) => 0xFB1F,
        (0x05B7, 0x05D0) => 0xFB2E,
        (0x05B8, 0x05D0) => 0xFB2F,
        (0x05B9, 0x05D5) => 0xFB4B,
        (0x05BC, 0x05D0..=0x05EA) => DAGESH_FORMS[(base - 0x05D0) as usize],
        (0x05BC, 0xFB2A) => 0xFB2C,
        (0x05BC, 0xFB2B) => 0xFB2D,
        (0x05BF, 0x05D1) => 0xFB4C,
        (0x05BF, 0x05DB) => 0xFB4D,
        (0x05BF, 0x05E4) => 0xFB4E,
        (0x05C1, 0x05E9) => 0xFB2A,
        (0x05C1, 0xFB49) => 0xFB2C,
        (0x05C2, 0x05E9) => 0xFB2B,
        (0x05C2, 0xFB49) => 0xFB2D,
        _ => 0,
    };
    (composed != 0).then_some(composed)
}

impl ComplexShaper for HebrewShaper {
    fn compose(&self, context: &NormalizeContext<'_>, first: u32, second: u32) -> Option<u32> {
        if let Some(composed) = rikka_text_unicode::compose(to_char(first), to_char(second)) {
            return Some(composed as u32);
        }
        if context.plan.has_gpos_mark {
            return None;
        }
        presentation_form(first, second)
    }

    fn gpos_tag(&self) -> Option<Tag> {
        Some(Tag::new(b"hebr"))
    }

    fn reorder_marks(&self, _: &PlanCore, buffer: &mut Buffer, start: usize, end: usize) {
        for index in start + 2..end {
            let first = buffer.info[index - 2].modified_combining_class();
            let second = buffer.info[index - 1].modified_combining_class();
            let third = buffer.info[index].modified_combining_class();
            if (first == HEBREW_PATAH || first == HEBREW_QAMATS)
                && (second == HEBREW_SHEVA || second == HEBREW_HIRIQ)
                && (third == HEBREW_METEG || third == BELOW)
            {
                buffer.merge_clusters(index - 1, index + 1);
                buffer.info.swap(index - 1, index);
                break;
            }
        }
    }
}

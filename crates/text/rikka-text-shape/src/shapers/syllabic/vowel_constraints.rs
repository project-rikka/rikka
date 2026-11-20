use rikka_text_unicode::Script;

use crate::buffer::glyph_buffer::Buffer;
use crate::common::buffer_flags::BufferFlags;

const DOTTED_CIRCLE: u32 = 0x25CC;

type Constraint = (u32, &'static [u32]);

const DEVANAGARI: &[Constraint] = &[
    (
        0x0905,
        &[
            0x093A, 0x093B, 0x093E, 0x0945, 0x0946, 0x0949, 0x094A, 0x094B, 0x094C, 0x094F, 0x0956,
            0x0957,
        ],
    ),
    (0x0906, &[0x093A, 0x0945, 0x0946, 0x0947, 0x0948]),
    (0x0909, &[0x0941]),
    (0x090F, &[0x0945, 0x0946, 0x0947]),
];

const BENGALI: &[Constraint] = &[
    (0x0985, &[0x09BE]),
    (0x098B, &[0x09C3]),
    (0x098C, &[0x09E2]),
];

const GURMUKHI: &[Constraint] = &[
    (0x0A05, &[0x0A3E, 0x0A48, 0x0A4C]),
    (0x0A72, &[0x0A3F, 0x0A40, 0x0A47]),
    (0x0A73, &[0x0A41, 0x0A42, 0x0A4B]),
];

const GUJARATI: &[Constraint] = &[
    (
        0x0A85,
        &[0x0ABE, 0x0AC5, 0x0AC7, 0x0AC8, 0x0AC9, 0x0ACB, 0x0ACC],
    ),
    (0x0AC5, &[0x0ABE]),
];

const ORIYA: &[Constraint] = &[
    (0x0B05, &[0x0B3E]),
    (0x0B0F, &[0x0B57]),
    (0x0B13, &[0x0B57]),
];

const TAMIL: &[Constraint] = &[(0x0B85, &[0x0BC2])];

const TELUGU: &[Constraint] = &[
    (0x0C12, &[0x0C4C, 0x0C55]),
    (0x0C3F, &[0x0C55]),
    (0x0C46, &[0x0C55]),
    (0x0C4A, &[0x0C55]),
];

const KANNADA: &[Constraint] = &[
    (0x0C89, &[0x0CBE]),
    (0x0C8B, &[0x0CBE]),
    (0x0C92, &[0x0CCC]),
];

const MALAYALAM: &[Constraint] = &[
    (0x0D07, &[0x0D57]),
    (0x0D09, &[0x0D57]),
    (0x0D0E, &[0x0D46]),
    (0x0D12, &[0x0D3E, 0x0D57]),
];

const SINHALA: &[Constraint] = &[
    (0x0D85, &[0x0DCF, 0x0DD0, 0x0DD1]),
    (0x0D8B, &[0x0DDF]),
    (0x0D8D, &[0x0DD8]),
    (0x0D8F, &[0x0DDF]),
    (0x0D91, &[0x0DCA, 0x0DD9, 0x0DDA, 0x0DDC, 0x0DDD, 0x0DDE]),
    (0x0D94, &[0x0DDF]),
];

const BRAHMI: &[Constraint] = &[
    (0x11005, &[0x11038]),
    (0x1100B, &[0x1103E]),
    (0x1100F, &[0x11042]),
];

const KHOJKI: &[Constraint] = &[
    (0x11200, &[0x1122C, 0x11231, 0x11233]),
    (0x11206, &[0x1122C]),
    (0x1122C, &[0x11230, 0x11231]),
    (0x11240, &[0x1122E]),
];

const KHUDAWADI: &[Constraint] = &[(0x112B0, &[0x112E0, 0x112E5, 0x112E6, 0x112E7, 0x112E8])];

const TIRHUTA: &[Constraint] = &[
    (0x11481, &[0x114B0]),
    (0x1148B, &[0x114BA]),
    (0x1148D, &[0x114BA]),
    (0x114AA, &[0x114B5, 0x114B6]),
];

const MODI: &[Constraint] = &[
    (0x11600, &[0x11639, 0x1163A]),
    (0x11601, &[0x11639, 0x1163A]),
];

const TAKRI: &[Constraint] = &[
    (0x11680, &[0x116AD, 0x116B4, 0x116B5]),
    (0x11686, &[0x116B2]),
];

const DEVANAGARI_RA: u32 = 0x0930;
const DEVANAGARI_VIRAMA: u32 = 0x094D;
const DEVANAGARI_I: u32 = 0x0907;

fn constraints(script: Script) -> Option<&'static [Constraint]> {
    Some(match script {
        Script::Devanagari => DEVANAGARI,
        Script::Bengali => BENGALI,
        Script::Gurmukhi => GURMUKHI,
        Script::Gujarati => GUJARATI,
        Script::Oriya => ORIYA,
        Script::Tamil => TAMIL,
        Script::Telugu => TELUGU,
        Script::Kannada => KANNADA,
        Script::Malayalam => MALAYALAM,
        Script::Sinhala => SINHALA,
        Script::Brahmi => BRAHMI,
        Script::Khojki => KHOJKI,
        Script::Khudawadi => KHUDAWADI,
        Script::Tirhuta => TIRHUTA,
        Script::Modi => MODI,
        Script::Takri => TAKRI,
        _ => return None,
    })
}

fn output_dotted_circle(buffer: &mut Buffer) {
    buffer.output_glyph(DOTTED_CIRCLE);
    buffer.prev_mut().clear_continuation();
}

fn is_reph_before_letter_i(buffer: &Buffer) -> bool {
    buffer.script == Script::Devanagari
        && buffer.idx + 2 < buffer.len
        && buffer.cur(0).codepoint == DEVANAGARI_RA
        && buffer.cur(1).codepoint == DEVANAGARI_VIRAMA
        && buffer.cur(2).codepoint == DEVANAGARI_I
}

pub(crate) fn insert_dotted_circles_in_invalid_vowels(buffer: &mut Buffer) {
    if buffer
        .flags
        .contains(BufferFlags::DO_NOT_INSERT_DOTTED_CIRCLE)
    {
        return;
    }
    let Some(constraints) = constraints(buffer.script) else {
        return;
    };
    buffer.clear_output();
    buffer.idx = 0;
    while buffer.idx + 1 < buffer.len && buffer.successful {
        if is_reph_before_letter_i(buffer) {
            buffer.next_glyph();
            buffer.next_glyph();
            output_dotted_circle(buffer);
            buffer.next_glyph();
            continue;
        }
        let (first, second) = (buffer.cur(0).codepoint, buffer.cur(1).codepoint);
        let invalid = constraints
            .iter()
            .any(|(vowel, signs)| *vowel == first && signs.contains(&second));
        buffer.next_glyph();
        if invalid {
            output_dotted_circle(buffer);
            buffer.next_glyph();
        }
    }
    buffer.sync();
}

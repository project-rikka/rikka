use rikka_text_font::OutlineScratch;
use rikka_text_unicode::GeneralCategory;

use crate::buffer::glyph_buffer::Buffer;
use crate::buffer::glyph_info::GlyphInfo;
use crate::buffer::glyph_position::GlyphPosition;
use crate::common::direction::Direction;
use crate::face::font_context::{FontContext, GlyphExtents};
use crate::unicode::combining_class as class;

fn recategorize_combining_class(codepoint: u32, combining_class: u8) -> u8 {
    if combining_class >= 200 {
        return combining_class;
    }
    let mut combining_class = combining_class;
    if codepoint & !0xFF == 0x0E00 {
        if combining_class == 0 {
            match codepoint {
                0x0E31 | 0x0E34..=0x0E37 | 0x0E47 | 0x0E4C..=0x0E4E => {
                    combining_class = class::ABOVE_RIGHT;
                }
                0x0EB1 | 0x0EB4..=0x0EB7 | 0x0EBB | 0x0ECC | 0x0ECD => {
                    combining_class = class::ABOVE;
                }
                0x0EBC => combining_class = class::BELOW,
                _ => {}
            }
        } else if codepoint == 0x0E3A {
            combining_class = class::BELOW_RIGHT;
        }
    }
    match combining_class {
        class::HEBREW_SHEVA
        | class::HEBREW_HATAF_SEGOL
        | class::HEBREW_HATAF_PATAH
        | class::HEBREW_HATAF_QAMATS
        | class::HEBREW_HIRIQ
        | class::HEBREW_TSERE
        | class::HEBREW_SEGOL
        | class::HEBREW_PATAH
        | class::HEBREW_QAMATS
        | class::HEBREW_QUBUTS
        | class::HEBREW_METEG => class::BELOW,
        class::HEBREW_RAFE => class::ATTACHED_ABOVE,
        class::HEBREW_SHIN_DOT => class::ABOVE_RIGHT,
        class::HEBREW_SIN_DOT | class::HEBREW_HOLAM => class::ABOVE_LEFT,
        class::HEBREW_POINT_VARIKA => class::ABOVE,
        class::ARABIC_FATHATAN
        | class::ARABIC_DAMMATAN
        | class::ARABIC_FATHA
        | class::ARABIC_DAMMA
        | class::ARABIC_SHADDA
        | class::ARABIC_SUKUN
        | class::ARABIC_SUPERSCRIPT_ALEF
        | class::SYRIAC_SUPERSCRIPT_ALAPH => class::ABOVE,
        class::ARABIC_KASRATAN | class::ARABIC_KASRA => class::BELOW,
        class::THAI_SARA_U => class::BELOW_RIGHT,
        class::THAI_MAI => class::ABOVE_RIGHT,
        class::LAO_SIGN_U => class::BELOW,
        class::LAO_MAI => class::ABOVE,
        class::TIBETAN_SIGN_AA | class::TIBETAN_SIGN_U => class::BELOW,
        class::TIBETAN_SIGN_I => class::ABOVE,
        _ => combining_class,
    }
}

pub(crate) fn recategorize_marks(buffer: &mut Buffer) {
    for info in &mut buffer.info[..buffer.len] {
        if info.general_category() == GeneralCategory::NonspacingMark {
            let combining_class =
                recategorize_combining_class(info.codepoint, info.modified_combining_class());
            info.set_modified_combining_class(combining_class);
        }
    }
}

fn zero_mark_advances(buffer: &mut Buffer, start: usize, end: usize, adjust_offsets: bool) {
    for (info, position) in buffer.info[start..end]
        .iter()
        .zip(&mut buffer.pos[start..end])
    {
        if info.general_category() == GeneralCategory::NonspacingMark {
            if adjust_offsets {
                position.x_offset -= position.x_advance;
                position.y_offset -= position.y_advance;
            }
            position.x_advance = 0;
            position.y_advance = 0;
        }
    }
}

fn position_mark(
    font: &FontContext,
    scratch: &mut OutlineScratch,
    direction: Direction,
    glyph: u32,
    position: &mut GlyphPosition,
    base: &mut GlyphExtents,
    combining_class: u8,
) {
    let Some(mark) = font.extents(glyph, scratch) else {
        return;
    };
    let y_gap = font.units_per_em() / 16;
    position.x_offset = 0;
    position.y_offset = 0;
    match combining_class {
        class::DOUBLE_BELOW | class::DOUBLE_ABOVE => {
            position.x_offset += base.x_bearing
                + if direction.is_forward() {
                    base.width
                } else {
                    0
                }
                - mark.width / 2
                - mark.x_bearing;
        }
        class::ATTACHED_BELOW_LEFT | class::BELOW_LEFT | class::ABOVE_LEFT => {
            position.x_offset += base.x_bearing - mark.x_bearing;
        }
        class::ATTACHED_ABOVE_RIGHT | class::BELOW_RIGHT | class::ABOVE_RIGHT => {
            position.x_offset += base.x_bearing + base.width - mark.width - mark.x_bearing;
        }
        _ => {
            position.x_offset += base.x_bearing + (base.width - mark.width) / 2 - mark.x_bearing;
        }
    }
    let attached = matches!(
        combining_class,
        class::ATTACHED_BELOW_LEFT
            | class::ATTACHED_BELOW
            | class::ATTACHED_ABOVE
            | class::ATTACHED_ABOVE_RIGHT
    );
    match combining_class {
        class::DOUBLE_BELOW
        | class::BELOW_LEFT
        | class::BELOW
        | class::BELOW_RIGHT
        | class::ATTACHED_BELOW_LEFT
        | class::ATTACHED_BELOW => {
            if !attached {
                base.height -= y_gap;
            }
            position.y_offset = base.y_bearing + base.height - mark.y_bearing;
            if (y_gap > 0) == (position.y_offset > 0) {
                base.height -= position.y_offset;
                position.y_offset = 0;
            }
            base.height += mark.height;
        }
        class::DOUBLE_ABOVE
        | class::ABOVE_LEFT
        | class::ABOVE
        | class::ABOVE_RIGHT
        | class::ATTACHED_ABOVE
        | class::ATTACHED_ABOVE_RIGHT => {
            if !attached {
                base.y_bearing += y_gap;
                base.height -= y_gap;
            }
            position.y_offset = base.y_bearing - (mark.y_bearing + mark.height);
            if (y_gap > 0) != (position.y_offset > 0) {
                let correction = -position.y_offset / 2;
                base.y_bearing += correction;
                base.height -= correction;
                position.y_offset += correction;
            }
            base.y_bearing -= mark.height;
            base.height += mark.height;
        }
        _ => {}
    }
}

fn position_around_base(
    font: &FontContext,
    buffer: &mut Buffer,
    (base, end): (usize, usize),
    layout_direction: Direction,
    adjust_offsets: bool,
) {
    buffer.unsafe_to_break(base, end);
    let base_info = buffer.info[base];
    let base_position = buffer.pos[base];
    let Some(mut base_extents) = font.extents(base_info.codepoint, &mut buffer.outline_scratch)
    else {
        zero_mark_advances(buffer, base + 1, end, adjust_offsets);
        return;
    };
    base_extents.y_bearing += base_position.y_offset;
    base_extents.x_bearing = 0;
    base_extents.width = font.advance(base_info.codepoint);
    let lig_id = base_info.lig_id();
    let component_count = i32::from(base_info.lig_num_comps());
    let forward = buffer.direction.is_forward();
    let (mut x_offset, mut y_offset) = if forward {
        (-base_position.x_advance, -base_position.y_advance)
    } else {
        (0, 0)
    };
    let mut last_component = -1;
    let mut last_class = 255;
    let mut component_extents = base_extents;
    let mut cluster_extents = base_extents;
    let direction = buffer.direction;
    for index in base + 1..end {
        let info = buffer.info[index];
        let combining_class = info.modified_combining_class();
        if combining_class != 0 {
            if component_count > 1 {
                let mut component = i32::from(info.lig_comp()) - 1;
                if lig_id == 0 || lig_id != info.lig_id() || component >= component_count {
                    component = component_count - 1;
                }
                if last_component != component {
                    last_component = component;
                    last_class = 255;
                    component_extents = base_extents;
                    let index_from_left = if layout_direction.is_forward() {
                        component
                    } else {
                        component_count - 1 - component
                    };
                    component_extents.x_bearing +=
                        (index_from_left * component_extents.width) / component_count;
                    component_extents.width /= component_count;
                }
            }
            if last_class != combining_class {
                last_class = combining_class;
                cluster_extents = component_extents;
            }
            let mut position = buffer.pos[index];
            position_mark(
                font,
                &mut buffer.outline_scratch,
                direction,
                info.codepoint,
                &mut position,
                &mut cluster_extents,
                combining_class,
            );
            position.x_advance = 0;
            position.y_advance = 0;
            position.x_offset += x_offset;
            position.y_offset += y_offset;
            buffer.pos[index] = position;
        } else if forward {
            x_offset -= buffer.pos[index].x_advance;
            y_offset -= buffer.pos[index].y_advance;
        } else {
            x_offset += buffer.pos[index].x_advance;
            y_offset += buffer.pos[index].y_advance;
        }
    }
}

fn is_attached_to_previous(info: &GlyphInfo) -> bool {
    info.is_unicode_mark() || info.is_hidden() || info.is_default_ignorable()
}

fn position_cluster(
    font: &FontContext,
    buffer: &mut Buffer,
    (start, end): (usize, usize),
    layout_direction: Direction,
    adjust_offsets: bool,
) {
    if end - start < 2 {
        return;
    }
    let mut index = start;
    while index < end {
        if !buffer.info[index].is_unicode_mark() {
            let mut mark_end = index + 1;
            while mark_end < end && is_attached_to_previous(&buffer.info[mark_end]) {
                mark_end += 1;
            }
            position_around_base(
                font,
                buffer,
                (index, mark_end),
                layout_direction,
                adjust_offsets,
            );
            index = mark_end - 1;
        }
        index += 1;
    }
}

pub(crate) fn position_marks(
    font: &FontContext,
    buffer: &mut Buffer,
    layout_direction: Direction,
    adjust_offsets: bool,
) {
    let len = buffer.len;
    let mut start = 0;
    for index in 1..len {
        if !is_attached_to_previous(&buffer.info[index]) {
            position_cluster(
                font,
                buffer,
                (start, index),
                layout_direction,
                adjust_offsets,
            );
            start = index;
        }
    }
    position_cluster(font, buffer, (start, len), layout_direction, adjust_offsets);
}

use rikka_text_unicode::GeneralCategory;

use crate::buffer::clusters::same_grapheme;
use crate::buffer::glyph_buffer::Buffer;
use crate::buffer::glyph_info::GlyphInfo;
use crate::buffer::scratch_flags;
use crate::common::buffer_flags::BufferFlags;
use crate::common::direction::Direction;
use crate::face::font_context::FontContext;
use crate::unicode::general_category::category_flag;
use crate::unicode::to_char;

const DOTTED_CIRCLE: u32 = 0x25CC;

const NON_CONTINUING_CATEGORIES: u32 = category_flag(GeneralCategory::LowercaseLetter)
    | category_flag(GeneralCategory::UppercaseLetter)
    | category_flag(GeneralCategory::TitlecaseLetter)
    | category_flag(GeneralCategory::OtherLetter)
    | category_flag(GeneralCategory::SpaceSeparator);

#[inline]
fn is_regional_indicator(codepoint: u32) -> bool {
    matches!(codepoint, 0x1F1E6..=0x1F1FF)
}

pub(crate) fn set_unicode_props(buffer: &mut Buffer) {
    let len = buffer.len;
    let mut scratch = buffer.scratch_flags;
    let mut index = 0;
    while index < len {
        buffer.info[index].init_unicode_props(&mut scratch);
        let codepoint = buffer.info[index].codepoint;
        if codepoint < 0x80 {
            index += 1;
            continue;
        }
        let category = buffer.info[index].general_category();
        if category_flag(category) & NON_CONTINUING_CATEGORIES != 0 {
            index += 1;
            continue;
        }
        if category == GeneralCategory::ModifierSymbol && matches!(codepoint, 0x1F3FB..=0x1F3FF) {
            buffer.info[index].set_continuation(&mut scratch);
        } else if index != 0 && is_regional_indicator(codepoint) {
            let previous = buffer.info[index - 1];
            if is_regional_indicator(previous.codepoint) && !previous.is_continuation() {
                buffer.info[index].set_continuation(&mut scratch);
            }
        } else if buffer.info[index].is_zwj() {
            buffer.info[index].set_continuation(&mut scratch);
            if index + 1 < len
                && rikka_text_unicode::is_extended_pictographic(to_char(
                    buffer.info[index + 1].codepoint,
                ))
            {
                index += 1;
                buffer.info[index].init_unicode_props(&mut scratch);
                buffer.info[index].set_continuation(&mut scratch);
            }
        } else if matches!(codepoint, 0xFF9E..=0xFF9F | 0xE0020..=0xE007F) {
            buffer.info[index].set_continuation(&mut scratch);
        } else if codepoint == 0x2044 {
            scratch |= scratch_flags::HAS_FRACTION_SLASH;
        }
        index += 1;
    }
    buffer.scratch_flags = scratch;
}

pub(crate) fn insert_dotted_circle(buffer: &mut Buffer, font: &FontContext) {
    if buffer
        .flags
        .contains(BufferFlags::DO_NOT_INSERT_DOTTED_CIRCLE)
        || !buffer.flags.contains(BufferFlags::BEGINNING_OF_TEXT)
        || buffer.context_len[0] != 0
        || !buffer.info[0].is_unicode_mark()
        || !font.has_glyph(DOTTED_CIRCLE)
    {
        return;
    }
    let mut info = GlyphInfo::new(DOTTED_CIRCLE, buffer.info[0].cluster);
    info.mask = buffer.info[0].mask;
    let mut scratch = buffer.scratch_flags;
    info.init_unicode_props(&mut scratch);
    buffer.scratch_flags = scratch;
    buffer.clear_output();
    buffer.output_info(info);
    buffer.sync();
}

pub(crate) fn form_clusters(buffer: &mut Buffer) {
    if buffer.scratch_flags & scratch_flags::HAS_CONTINUATIONS == 0 {
        return;
    }
    let mut start = 0;
    while start < buffer.len {
        let end = buffer.group_end(start, same_grapheme);
        buffer.merge_clusters(start, end);
        start = end;
    }
}

pub(crate) fn ensure_native_direction(buffer: &mut Buffer) {
    let direction = buffer.direction;
    let mut native = Direction::from_script(buffer.script);
    if native == Some(Direction::RightToLeft) && direction == Direction::LeftToRight {
        let mut found_number = false;
        let mut found_letter = false;
        let mut found_regional_indicator = false;
        for info in &buffer.info[..buffer.len] {
            let category = info.general_category();
            if category == GeneralCategory::DecimalNumber {
                found_number = true;
            } else if category.is_letter() {
                found_letter = true;
                break;
            } else if is_regional_indicator(info.codepoint) {
                found_regional_indicator = true;
            }
        }
        if (found_number || found_regional_indicator) && !found_letter {
            native = Some(Direction::LeftToRight);
        }
    }
    if native.is_some_and(|native| native != direction) {
        buffer.reverse_groups(same_grapheme);
        buffer.direction = direction.reversed();
    }
}

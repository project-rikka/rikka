use rikka_text_unicode::GeneralCategory;

use crate::buffer::glyph_buffer::Buffer;
use crate::buffer::scratch_flags;
use crate::face::font_context::FontContext;
use crate::shapers::arabic::joining::action;
use crate::unicode::general_category::category_flag;

const HAS_STRETCH: u32 = scratch_flags::SHAPER0;

const WORD_CATEGORIES: u32 = category_flag(GeneralCategory::Unassigned)
    | category_flag(GeneralCategory::PrivateUse)
    | category_flag(GeneralCategory::ModifierLetter)
    | category_flag(GeneralCategory::OtherLetter)
    | category_flag(GeneralCategory::SpacingMark)
    | category_flag(GeneralCategory::EnclosingMark)
    | category_flag(GeneralCategory::NonspacingMark)
    | category_flag(GeneralCategory::DecimalNumber)
    | category_flag(GeneralCategory::LetterNumber)
    | category_flag(GeneralCategory::OtherNumber)
    | category_flag(GeneralCategory::CurrencySymbol)
    | category_flag(GeneralCategory::ModifierSymbol)
    | category_flag(GeneralCategory::MathSymbol)
    | category_flag(GeneralCategory::OtherSymbol);

pub(crate) fn record_stretches(buffer: &mut Buffer) {
    let mut found = false;
    for info in &mut buffer.info[..buffer.len] {
        if info.multiplied() {
            info.shaper_auxiliary = if info.lig_comp() % 2 != 0 {
                action::STRETCHING_REPEATING
            } else {
                action::STRETCHING_FIXED
            };
            found = true;
        }
    }
    if found {
        buffer.scratch_flags |= HAS_STRETCH;
    }
}

fn is_stretching(buffer: &Buffer, index: usize) -> bool {
    action::is_stretching(buffer.info[index].shaper_auxiliary)
}

pub(crate) fn apply_stretches(font: &FontContext, buffer: &mut Buffer) {
    if buffer.scratch_flags & HAS_STRETCH == 0 {
        return;
    }
    let rtl = buffer.direction.is_backward();
    if !rtl {
        buffer.reverse();
    }
    let mut extra_glyphs = 0usize;
    for cutting in [false, true] {
        let new_len = buffer.len + extra_glyphs;
        let mut index = buffer.len;
        let mut write = new_len;
        while index != 0 {
            if !is_stretching(buffer, index - 1) {
                if cutting {
                    write -= 1;
                    buffer.info[write] = buffer.info[index - 1];
                    buffer.pos[write] = buffer.pos[index - 1];
                }
                index -= 1;
                continue;
            }
            let mut total = 0;
            let mut fixed = 0;
            let mut repeating = 0;
            let mut repeating_count: i32 = 0;
            let end = index;
            while index != 0 && is_stretching(buffer, index - 1) {
                index -= 1;
                let width = font.advance(buffer.info[index].codepoint);
                if buffer.info[index].shaper_auxiliary == action::STRETCHING_FIXED {
                    fixed += width;
                } else {
                    repeating += width;
                    repeating_count += 1;
                }
            }
            let start = index;
            let mut context = index;
            while context != 0
                && !is_stretching(buffer, context - 1)
                && (buffer.info[context - 1].is_default_ignorable()
                    || category_flag(buffer.info[context - 1].general_category()) & WORD_CATEGORIES
                        != 0)
            {
                context -= 1;
                total += buffer.pos[context].x_advance;
            }
            index += 1;
            let mut copies: i32 = 0;
            let mut remaining = total - fixed;
            if remaining > repeating && repeating > 0 {
                copies = remaining / repeating - 1;
            }
            let mut overlap = 0;
            let shortfall = remaining - repeating * (copies + 1);
            if shortfall > 0 && repeating_count > 0 {
                copies += 1;
                let excess = (copies + 1) * repeating - remaining;
                if excess > 0 {
                    overlap = excess / (copies * repeating_count);
                    remaining = 0;
                }
            }
            if cutting {
                buffer.unsafe_to_break(context, end);
                let mut x_offset = remaining / 2;
                for k in (start + 1..=end).rev() {
                    let width = font.advance(buffer.info[k - 1].codepoint);
                    let mut repeat = 1;
                    if buffer.info[k - 1].shaper_auxiliary == action::STRETCHING_REPEATING {
                        repeat += copies;
                    }
                    buffer.pos[k - 1].x_advance = 0;
                    for copy in 0..repeat {
                        if rtl {
                            x_offset -= width;
                            if copy > 0 {
                                x_offset += overlap;
                            }
                        }
                        buffer.pos[k - 1].x_offset = x_offset;
                        write -= 1;
                        buffer.info[write] = buffer.info[k - 1];
                        buffer.pos[write] = buffer.pos[k - 1];
                        if !rtl {
                            x_offset += width;
                            if copy > 0 {
                                x_offset -= overlap;
                            }
                        }
                    }
                }
            } else {
                extra_glyphs += (copies * repeating_count).max(0) as usize;
            }
            index -= 1;
        }
        if cutting {
            buffer.len = new_len;
        } else if !buffer.ensure(buffer.len + extra_glyphs) {
            break;
        }
    }
    if !rtl {
        buffer.reverse();
    }
}

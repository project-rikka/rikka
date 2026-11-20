use rikka_text_font::{GlyphId, KernSubtable};

use crate::buffer::glyph_buffer::Buffer;
use crate::buffer::glyph_position::attach_kind;
use crate::buffer::scratch_flags;
use crate::face::font_context::FontContext;
use crate::layout::apply_context::ApplyContext;
use crate::layout::matching::{AnyGlyph, lookup_flags};
use crate::plan::feature_map::TableKind;
use crate::plan::shape_plan::PlanCore;

pub(crate) fn apply_kern_table(plan: &PlanCore, font: &FontContext, buffer: &mut Buffer) {
    let Some(kern) = font.font.kern() else {
        return;
    };
    let mut reversed = false;
    let mut seen_cross_stream = false;
    for subtable in kern.subtables() {
        if subtable.is_variation() || !subtable.is_horizontal() {
            continue;
        }
        let cross_stream = subtable.is_cross_stream();
        if !seen_cross_stream && cross_stream {
            seen_cross_stream = true;
            let chain = if buffer.direction.is_forward() { -1 } else { 1 };
            for position in &mut buffer.pos[..buffer.len] {
                position.attach_kind = attach_kind::CURSIVE;
                position.attach_chain = chain;
            }
        }
        if buffer.direction.is_backward() != reversed {
            buffer.reverse();
            reversed = !reversed;
        }
        if plan.requested_kerning && matches!(subtable.format(), 0 | 2) {
            kern_pairs(plan, font, buffer, &subtable, cross_stream);
        }
    }
    if reversed {
        buffer.reverse();
    }
}

fn kern_pairs(
    plan: &PlanCore,
    font: &FontContext,
    buffer: &mut Buffer,
    subtable: &KernSubtable<'_>,
    cross_stream: bool,
) {
    buffer.unsafe_to_concat_all();
    let mut ctx = ApplyContext::new(TableKind::Gpos, font, buffer);
    ctx.set_lookup_mask(plan.kern_mask);
    ctx.lookup_props = lookup_flags::IGNORE_MARKS;
    ctx.update_matchers();
    let mut iterator = ctx.input_iterator();
    let mut index = 0;
    while index < ctx.buffer.len {
        if ctx.buffer.info[index].mask & plan.kern_mask == 0 {
            index += 1;
            continue;
        }
        iterator.reset_fast(index);
        let mut unsafe_to = 0;
        if !iterator.next(ctx.buffer, font, &AnyGlyph, &mut unsafe_to) {
            index += 1;
            continue;
        }
        let next = iterator.index;
        let left = GlyphId(ctx.buffer.info[index].codepoint as u16);
        let right = GlyphId(ctx.buffer.info[next].codepoint as u16);
        let value = subtable.kerning(left, right).unwrap_or(0);
        if value != 0 {
            if cross_stream {
                ctx.buffer.pos[next].y_offset = value;
                ctx.buffer.scratch_flags |= scratch_flags::HAS_GPOS_ATTACHMENT;
            } else {
                let first_half = value >> 1;
                let second_half = value - first_half;
                ctx.buffer.pos[index].x_advance += first_half;
                ctx.buffer.pos[next].x_advance += second_half;
                ctx.buffer.pos[next].x_offset += second_half;
            }
            ctx.buffer.unsafe_to_break(index, next + 1);
        }
        index = next;
    }
}

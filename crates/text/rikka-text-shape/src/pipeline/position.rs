use crate::buffer::glyph_buffer::Buffer;
use crate::buffer::scratch_flags;
use crate::face::font_context::FontContext;
use crate::layout::attachment::{clear_attachments, propagate_attachment_offsets};
use crate::layout::dispatch::apply_layout_table;
use crate::pipeline::fallback_marks::position_marks;
use crate::pipeline::fallback_spaces::position_fallback_spaces;
use crate::pipeline::ignorables::zero_width_default_ignorables;
use crate::pipeline::kern_table::apply_kern_table;
use crate::plan::feature_map::TableKind;
use crate::plan::shape_plan::PlanCore;

pub(crate) fn position(plan: &PlanCore, font: &FontContext, buffer: &mut Buffer) {
    buffer.clear_positions();
    position_default(font, buffer);
    position_complex(plan, font, buffer);
    if buffer.direction.is_backward() {
        buffer.reverse();
    }
}

fn position_default(font: &FontContext, buffer: &mut Buffer) {
    let len = buffer.len;
    font.fill_advances(&buffer.info[..len], &mut buffer.pos[..len]);
    if buffer.scratch_flags & scratch_flags::HAS_SPACE_FALLBACK != 0 {
        position_fallback_spaces(font, buffer);
    }
}

fn zero_mark_widths_by_gdef(buffer: &mut Buffer, adjust_offsets: bool) {
    let len = buffer.len;
    for (info, position) in buffer.info[..len].iter().zip(&mut buffer.pos[..len]) {
        if info.is_mark() {
            if adjust_offsets {
                position.x_offset -= position.x_advance;
                position.y_offset -= position.y_advance;
            }
            position.x_advance = 0;
            position.y_advance = 0;
        }
    }
}

fn position_complex(plan: &PlanCore, font: &FontContext, buffer: &mut Buffer) {
    let adjust_offsets = plan.adjust_mark_positioning_when_zeroing && buffer.direction.is_forward();
    let zero_widths = plan.shaper.zero_width_marks();
    clear_attachments(buffer);
    if plan.zero_marks && zero_widths.zeroes_before_positioning() {
        zero_mark_widths_by_gdef(buffer, adjust_offsets);
    }
    if plan.apply_gpos {
        apply_layout_table(plan, font, buffer, TableKind::Gpos);
    }
    if plan.apply_kern {
        apply_kern_table(plan, font, buffer);
    }
    if plan.zero_marks && zero_widths.zeroes_after_positioning() {
        zero_mark_widths_by_gdef(buffer, adjust_offsets);
    }
    zero_width_default_ignorables(buffer);
    propagate_attachment_offsets(buffer);
    if plan.fallback_mark_positioning {
        position_marks(font, buffer, plan.properties.direction, adjust_offsets);
    }
}

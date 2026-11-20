use rikka_text_font::{
    Anchor, Array, ChainedSequenceContext, CursivePos, GlyphId, LookupSubtable, MarkArray,
    MarkBasePos, MarkLigPos, MarkMarkPos, PairPos, PosSubtable, SequenceContext, SinglePos,
    ValueFormat, ValueRecord,
};

use crate::buffer::glyph_buffer::Buffer;
use crate::buffer::glyph_position::{GlyphPosition, attach_kind};
use crate::buffer::scratch_flags;
use crate::face::font_context::FontContext;
use crate::face::lookup_accel::{LookupAccel, SubtableAccel};
use crate::layout::apply_context::ApplyContext;
use crate::layout::chain_context::{apply_chain_context, apply_chain_sequence};
use crate::layout::contextual::{apply_context, apply_context_sequence};
use crate::layout::matching::{AnyGlyph, lookup_flags};
use crate::layout::pair_second::PairSecond;

pub(crate) fn apply_pos_subtable(
    ctx: &mut ApplyContext<'_>,
    table: &[u8],
    lookup: &LookupAccel,
    accel: &SubtableAccel,
    coverage_index: u16,
    second: &mut PairSecond,
) -> bool {
    let Some(location) = accel.location else {
        return false;
    };
    if let Some(sequence) = &accel.sequence {
        let records = || match PosSubtable::from_location(table, location) {
            Some(PosSubtable::Context(SequenceContext::Coverages(context))) => context.lookups(),
            Some(PosSubtable::ChainContext(ChainedSequenceContext::Coverages(context))) => {
                context.lookups()
            }
            _ => Array::default(),
        };
        return if sequence.chained() {
            apply_chain_sequence(ctx, &lookup.sets, sequence, records)
        } else {
            apply_context_sequence(ctx, &lookup.sets, sequence, records)
        };
    }
    let Some(subtable) = PosSubtable::from_location(table, location) else {
        return false;
    };
    match &subtable {
        PosSubtable::Single(single) => apply_single(ctx, single, coverage_index),
        PosSubtable::Pair(pair) => apply_pair(ctx, pair, accel, coverage_index, second),
        PosSubtable::Cursive(cursive) => apply_cursive(ctx, cursive, accel, coverage_index),
        PosSubtable::MarkBase(mark_base) => apply_mark_base(ctx, mark_base, accel, coverage_index),
        PosSubtable::MarkLigature(mark_ligature) => {
            apply_mark_ligature(ctx, mark_ligature, accel, coverage_index)
        }
        PosSubtable::MarkMark(mark_mark) => apply_mark_mark(ctx, mark_mark, accel, coverage_index),
        PosSubtable::Context(context) => apply_context(ctx, context, accel, coverage_index),
        PosSubtable::ChainContext(context) => {
            apply_chain_context(ctx, context, accel, coverage_index)
        }
    }
}

#[inline]
fn glyph_at(buffer: &Buffer, index: usize) -> GlyphId {
    GlyphId(buffer.info[index].codepoint as u16)
}

fn apply_value(font: &FontContext, position: &mut GlyphPosition, value: &ValueRecord<'_>) -> bool {
    let format = value.format();
    let mut applied = false;
    let mut add = |target: &mut i32, field: u16, amount: i32| {
        if format.contains(field) {
            applied |= amount != 0;
            *target += amount;
        }
    };
    add(
        &mut position.x_offset,
        ValueFormat::X_PLACEMENT,
        i32::from(value.x_placement()),
    );
    add(
        &mut position.y_offset,
        ValueFormat::Y_PLACEMENT,
        i32::from(value.y_placement()),
    );
    add(
        &mut position.x_advance,
        ValueFormat::X_ADVANCE,
        i32::from(value.x_advance()),
    );
    if format.0 & 0x00F0 == 0 || !font.has_layout_variations() {
        return applied;
    }
    let mut add_delta = |target: &mut i32, field: u16, device| {
        if format.contains(field) {
            let amount = font.device_delta(device);
            applied |= amount != 0;
            *target += amount;
        }
    };
    add_delta(
        &mut position.x_offset,
        ValueFormat::X_PLACEMENT_DEVICE,
        value.x_placement_device(),
    );
    add_delta(
        &mut position.y_offset,
        ValueFormat::Y_PLACEMENT_DEVICE,
        value.y_placement_device(),
    );
    add_delta(
        &mut position.x_advance,
        ValueFormat::X_ADVANCE_DEVICE,
        value.x_advance_device(),
    );
    applied
}

#[inline]
fn resolve_anchor(font: &FontContext, anchor: &Anchor<'_>) -> (i32, i32) {
    let mut x = i32::from(anchor.x());
    let mut y = i32::from(anchor.y());
    if font.has_layout_variations() {
        x += font.device_delta(anchor.x_device());
        y += font.device_delta(anchor.y_device());
    }
    (x, y)
}

fn apply_single(ctx: &mut ApplyContext<'_>, single: &SinglePos<'_>, coverage_index: u16) -> bool {
    let index = ctx.buffer.idx;
    let Some(value) = single.value_at(coverage_index) else {
        return false;
    };
    apply_value(ctx.font, &mut ctx.buffer.pos[index], &value);
    ctx.buffer.idx += 1;
    true
}

fn apply_pair(
    ctx: &mut ApplyContext<'_>,
    pair: &PairPos<'_>,
    accel: &SubtableAccel,
    coverage_index: u16,
    second: &mut PairSecond,
) -> bool {
    let first_index = ctx.buffer.idx;
    let first = glyph_at(ctx.buffer, first_index);
    let second_index = match second.find(ctx) {
        Ok(index) => index,
        Err(unsafe_to) => {
            ctx.buffer.unsafe_to_concat(first_index, unsafe_to);
            return false;
        }
    };
    let second = glyph_at(ctx.buffer, second_index);
    let values = match pair {
        PairPos::Glyphs(glyphs) => {
            match glyphs.pair_set(coverage_index).and_then(|set| {
                if accel.has_ordered_pairs(coverage_index) {
                    set.find_ordered(second)
                } else {
                    set.find(second)
                }
            }) {
                Some(values) => values,
                None => return false,
            }
        }
        PairPos::Classes(classes) => {
            let first_class = accel.class(0, u32::from(first.0));
            let second_class = accel.class(1, u32::from(second.0));
            match classes.class_values(first_class, second_class) {
                Some(values) => values,
                None => {
                    ctx.buffer.unsafe_to_concat(first_index, second_index + 1);
                    return false;
                }
            }
        }
    };
    let (format1, format2) = pair.value_formats();
    let has_record2 = !format2.is_empty();
    let (value1, value2) = values;
    let applied_first =
        !format1.is_empty() && apply_value(ctx.font, &mut ctx.buffer.pos[first_index], &value1);
    let applied_second =
        has_record2 && apply_value(ctx.font, &mut ctx.buffer.pos[second_index], &value2);
    if applied_first || applied_second {
        ctx.buffer.unsafe_to_break(first_index, second_index + 1);
    } else {
        ctx.buffer.unsafe_to_concat(first_index, second_index + 1);
    }
    let mut next = second_index;
    if has_record2 {
        next += 1;
        ctx.buffer.unsafe_to_break(first_index, next + 1);
    }
    ctx.buffer.idx = next;
    true
}

fn apply_cursive(
    ctx: &mut ApplyContext<'_>,
    cursive: &CursivePos<'_>,
    accel: &SubtableAccel,
    coverage_index: u16,
) -> bool {
    let this_index = ctx.buffer.idx;
    let Some(this_record) = cursive.entry_exit_at(coverage_index) else {
        return false;
    };
    let Some(entry_this) = this_record.entry else {
        return false;
    };
    let mut iterator = ctx.input_iterator();
    iterator.reset_fast(this_index);
    let mut unsafe_from = 0;
    if !iterator.prev(ctx.buffer, ctx.font, &AnyGlyph, &mut unsafe_from) {
        ctx.buffer
            .unsafe_to_concat_from_outbuffer(unsafe_from, this_index + 1);
        return false;
    }
    let previous = iterator.index;
    let Some(previous_record) = accel
        .target_index(ctx.buffer.info[previous].codepoint)
        .and_then(|index| cursive.entry_exit_at(index))
    else {
        return false;
    };
    let Some(exit_previous) = previous_record.exit else {
        ctx.buffer
            .unsafe_to_concat_from_outbuffer(previous, this_index + 1);
        return false;
    };
    let (exit_x, exit_y) = resolve_anchor(ctx.font, &exit_previous);
    let (entry_x, entry_y) = resolve_anchor(ctx.font, &entry_this);
    let (i, j) = (previous, this_index);
    ctx.buffer.unsafe_to_break(i, j + 1);
    let forward = ctx.buffer.direction.is_forward();
    let positions = &mut ctx.buffer.pos;
    if forward {
        positions[i].x_advance = exit_x + positions[i].x_offset;
        let delta = entry_x + positions[j].x_offset;
        positions[j].x_advance -= delta;
        positions[j].x_offset -= delta;
    } else {
        let delta = exit_x + positions[i].x_offset;
        positions[i].x_advance -= delta;
        positions[i].x_offset -= delta;
        positions[j].x_advance = entry_x + positions[j].x_offset;
    }
    let (mut child, mut parent) = (i, j);
    let mut y_offset = entry_y - exit_y;
    if ctx.lookup_props & lookup_flags::RIGHT_TO_LEFT == 0 {
        std::mem::swap(&mut child, &mut parent);
        y_offset = -y_offset;
    }
    reverse_cursive_minor_offset(positions, child, parent);
    positions[child].attach_kind = attach_kind::CURSIVE;
    positions[child].attach_chain = (parent as isize - child as isize) as i16;
    positions[child].y_offset = y_offset;
    if positions[parent].attach_chain == -positions[child].attach_chain {
        positions[parent].attach_chain = 0;
        positions[parent].y_offset = 0;
    }
    ctx.buffer.scratch_flags |= scratch_flags::HAS_GPOS_ATTACHMENT;
    ctx.buffer.idx += 1;
    true
}

fn reverse_cursive_minor_offset(positions: &mut [GlyphPosition], index: usize, new_parent: usize) {
    let chain = positions[index].attach_chain;
    let kind = positions[index].attach_kind;
    if chain == 0 || kind & attach_kind::CURSIVE == 0 {
        return;
    }
    positions[index].attach_chain = 0;
    let parent = (index as isize + isize::from(chain)) as usize;
    if parent == new_parent {
        return;
    }
    reverse_cursive_minor_offset(positions, parent, new_parent);
    positions[parent].y_offset = -positions[index].y_offset;
    positions[parent].attach_chain = -chain;
    positions[parent].attach_kind = kind;
}

fn attach_mark(
    ctx: &mut ApplyContext<'_>,
    marks: &MarkArray<'_>,
    mark_index: u16,
    base_anchor: Option<Anchor<'_>>,
    target: usize,
) -> bool {
    let Some(record) = marks.get(mark_index) else {
        return false;
    };
    let Some(base_anchor) = base_anchor else {
        return false;
    };
    let (base_x, base_y) = resolve_anchor(ctx.font, &base_anchor);
    let (mark_x, mark_y) = resolve_anchor(ctx.font, &record.anchor);
    let idx = ctx.buffer.idx;
    ctx.buffer.unsafe_to_break(target, idx + 1);
    let position = &mut ctx.buffer.pos[idx];
    position.x_offset = base_x - mark_x;
    position.y_offset = base_y - mark_y;
    position.attach_kind = attach_kind::MARK;
    position.attach_chain = (target as isize - idx as isize) as i16;
    ctx.buffer.scratch_flags |= scratch_flags::HAS_GPOS_ATTACHMENT;
    ctx.buffer.idx += 1;
    true
}

fn accepts_base(buffer: &Buffer, index: usize) -> bool {
    let info = &buffer.info[index];
    !info.multiplied()
        || info.lig_comp() == 0
        || index == 0
        || buffer.info[index - 1].is_mark()
        || !buffer.info[index - 1].multiplied()
        || info.lig_id() != buffer.info[index - 1].lig_id()
        || info.lig_comp() != buffer.info[index - 1].lig_comp() + 1
}

fn find_last_base(
    ctx: &mut ApplyContext<'_>,
    accept: impl Fn(&Buffer, usize) -> bool,
) -> Option<usize> {
    let idx = ctx.buffer.idx;
    if ctx.last_base_until > idx {
        ctx.last_base_until = 0;
        ctx.last_base = None;
    }
    let mut iterator = ctx.input_iterator();
    iterator.lookup_props = lookup_flags::IGNORE_MARKS;
    let mut base = ctx.last_base;
    let mut index = idx;
    while index > ctx.last_base_until {
        let info = &ctx.buffer.info[index - 1];
        if iterator.matches_at(ctx.font, info, &AnyGlyph) && accept(ctx.buffer, index - 1) {
            base = Some(index - 1);
            break;
        }
        index -= 1;
    }
    ctx.last_base_until = idx;
    ctx.last_base = base;
    base
}

fn apply_mark_base(
    ctx: &mut ApplyContext<'_>,
    mark_base: &MarkBasePos<'_>,
    accel: &SubtableAccel,
    mark_index: u16,
) -> bool {
    let idx = ctx.buffer.idx;
    let Some(base) = find_last_base(ctx, |buffer, index| {
        accepts_base(buffer, index) || accel.target_index(buffer.info[index].codepoint).is_some()
    }) else {
        ctx.buffer.unsafe_to_concat_from_outbuffer(0, idx + 1);
        return false;
    };
    let Some(base_index) = accel.target_index(ctx.buffer.info[base].codepoint) else {
        ctx.buffer.unsafe_to_concat_from_outbuffer(base, idx + 1);
        return false;
    };
    let marks = mark_base.marks();
    let Some(record) = marks.get(mark_index) else {
        return false;
    };
    let anchor = mark_base
        .bases()
        .anchor(usize::from(base_index), record.class);
    attach_mark(ctx, &marks, mark_index, anchor, base)
}

fn apply_mark_ligature(
    ctx: &mut ApplyContext<'_>,
    mark_ligature: &MarkLigPos<'_>,
    accel: &SubtableAccel,
    mark_index: u16,
) -> bool {
    let idx = ctx.buffer.idx;
    let Some(base) = find_last_base(ctx, |_, _| true) else {
        ctx.buffer.unsafe_to_concat_from_outbuffer(0, idx + 1);
        return false;
    };
    let Some(ligature_index) = accel.target_index(ctx.buffer.info[base].codepoint) else {
        ctx.buffer.unsafe_to_concat_from_outbuffer(base, idx + 1);
        return false;
    };
    let Some(attach) = mark_ligature.ligature(ligature_index) else {
        return false;
    };
    let component_count = attach.rows();
    if component_count == 0 {
        ctx.buffer.unsafe_to_concat_from_outbuffer(base, idx + 1);
        return false;
    }
    let lig_id = ctx.buffer.info[base].lig_id();
    let mark_id = ctx.buffer.cur(0).lig_id();
    let mark_comp = usize::from(ctx.buffer.cur(0).lig_comp());
    let component = if lig_id != 0 && lig_id == mark_id && mark_comp > 0 {
        mark_comp.min(component_count)
    } else {
        component_count
    } - 1;
    let marks = mark_ligature.marks();
    let Some(record) = marks.get(mark_index) else {
        return false;
    };
    let anchor = attach.anchor(component, record.class);
    attach_mark(ctx, &marks, mark_index, anchor, base)
}

fn apply_mark_mark(
    ctx: &mut ApplyContext<'_>,
    mark_mark: &MarkMarkPos<'_>,
    accel: &SubtableAccel,
    mark1_index: u16,
) -> bool {
    let idx = ctx.buffer.idx;
    let mut iterator = ctx.input_iterator();
    iterator.reset_fast(idx);
    iterator.lookup_props = ctx.lookup_props & !lookup_flags::IGNORE_FLAGS;
    let mut unsafe_from = 0;
    if !iterator.prev(ctx.buffer, ctx.font, &AnyGlyph, &mut unsafe_from) {
        ctx.buffer
            .unsafe_to_concat_from_outbuffer(unsafe_from, idx + 1);
        return false;
    }
    let previous = iterator.index;
    if !ctx.buffer.info[previous].is_mark() {
        ctx.buffer
            .unsafe_to_concat_from_outbuffer(previous, idx + 1);
        return false;
    }
    let id1 = ctx.buffer.cur(0).lig_id();
    let id2 = ctx.buffer.info[previous].lig_id();
    let comp1 = ctx.buffer.cur(0).lig_comp();
    let comp2 = ctx.buffer.info[previous].lig_comp();
    let matches = if id1 == id2 {
        id1 == 0 || comp1 == comp2
    } else {
        (id1 > 0 && comp1 == 0) || (id2 > 0 && comp2 == 0)
    };
    if !matches {
        ctx.buffer
            .unsafe_to_concat_from_outbuffer(previous, idx + 1);
        return false;
    }
    let Some(mark2_index) = accel.target_index(ctx.buffer.info[previous].codepoint) else {
        return false;
    };
    let marks = mark_mark.marks();
    let Some(record) = marks.get(mark1_index) else {
        return false;
    };
    let anchor = mark_mark
        .base_marks()
        .anchor(usize::from(mark2_index), record.class);
    attach_mark(ctx, &marks, mark1_index, anchor, previous)
}

use crate::buffer::glyph_buffer::Buffer;
use crate::buffer::glyph_info::GlyphInfo;
use crate::face::candidates::Candidates;
use crate::face::font_context::FontContext;
use crate::face::lookup_accel::LookupAccel;
use crate::face::lookup_filter::LookupFilter;
use crate::layout::apply_context::ApplyContext;
use crate::layout::gpos::apply_pos_subtable;
use crate::layout::gsub::apply_subst_subtable;
use crate::layout::matching::check_glyph_property;
use crate::layout::pair_second::PairSecond;
use crate::plan::feature_map::{LookupEntry, TableKind};
use crate::plan::shape_plan::PlanCore;

pub(crate) fn apply_lookup_here(ctx: &mut ApplyContext<'_>, index: u16) -> bool {
    let Some(accel) = ctx.font.lookup(ctx.table, index) else {
        return false;
    };
    ctx.lookup_props = accel.filter.props;
    ctx.update_matchers();
    apply_lookup(ctx, accel)
}

fn apply_lookup(ctx: &mut ApplyContext<'_>, accel: &LookupAccel) -> bool {
    let Some(table) = ctx.font.table(ctx.table) else {
        return false;
    };
    let candidates = accel.candidates.get(ctx.buffer.cur(0).codepoint);
    match ctx.table {
        TableKind::Gsub => candidates.iter().any(|candidate| {
            apply_subst_subtable(
                ctx,
                table,
                accel,
                &accel.subtables[usize::from(candidate.subtable)],
                candidate.coverage_index,
            )
        }),
        TableKind::Gpos => {
            let mut second = PairSecond::default();
            candidates.iter().any(|candidate| {
                apply_pos_subtable(
                    ctx,
                    table,
                    accel,
                    &accel.subtables[usize::from(candidate.subtable)],
                    candidate.coverage_index,
                    &mut second,
                )
            })
        }
    }
}

pub(crate) fn apply_layout_table(
    plan: &PlanCore,
    font: &FontContext,
    buffer: &mut Buffer,
    table: TableKind,
) {
    let present = font.table(table).is_some();
    let mut ctx = ApplyContext::new(table, font, buffer);
    let mut masks = ctx.buffer.mask_union();
    for (stage_index, stage) in plan.map.stages(table).iter().enumerate() {
        if present {
            for entry in plan.map.stage_lookups(table, stage_index) {
                if entry.mask & masks == 0 || !entry.digest.may_intersect(&ctx.buffer.digest) {
                    continue;
                }
                let Some(accel) = font.lookup(table, entry.index) else {
                    continue;
                };
                prepare(&mut ctx, entry, &accel.filter);
                apply_string(&mut ctx, &accel.filter, Some(&accel.candidates), |ctx| {
                    apply_lookup(ctx, accel)
                });
            }
        }
        if let Some(pause) = stage.pause {
            if pause(plan, font, ctx.buffer) {
                ctx.buffer.update_digest();
            }
            masks = ctx.buffer.mask_union();
        }
    }
}

fn prepare(ctx: &mut ApplyContext<'_>, entry: &LookupEntry, filter: &LookupFilter) {
    ctx.lookup_index = entry.index;
    ctx.set_lookup_mask(entry.mask);
    ctx.auto_zwj = entry.auto_zwj;
    ctx.auto_zwnj = entry.auto_zwnj;
    ctx.random = entry.random;
    ctx.per_syllable = entry.per_syllable;
    ctx.lookup_props = filter.props;
}

pub(crate) fn apply_string(
    ctx: &mut ApplyContext<'_>,
    filter: &LookupFilter,
    candidates: Option<&Candidates>,
    mut apply: impl FnMut(&mut ApplyContext<'_>) -> bool,
) {
    if ctx.buffer.len == 0 || ctx.lookup_mask == 0 {
        return;
    }
    ctx.lookup_props = filter.props;
    ctx.update_matchers();
    let scan = Scan { filter, candidates };
    let in_place = ctx.table == TableKind::Gpos;
    if !filter.reverse {
        if !in_place {
            ctx.buffer.clear_output();
        }
        ctx.buffer.idx = 0;
        apply_forward(ctx, &scan, &mut apply);
        if !in_place {
            ctx.buffer.sync();
        }
    } else {
        ctx.buffer.idx = ctx.buffer.len - 1;
        apply_backward(ctx, &scan, &mut apply);
    }
}

struct Scan<'l> {
    filter: &'l LookupFilter,
    candidates: Option<&'l Candidates>,
}

impl Scan<'_> {
    #[inline(always)]
    fn applies_to(&self, ctx: &ApplyContext<'_>, info: &GlyphInfo) -> bool {
        self.filter.digest.may_have(info.codepoint)
            && info.mask & ctx.lookup_mask != 0
            && check_glyph_property(ctx.font, info, ctx.lookup_props)
            && self
                .candidates
                .is_none_or(|candidates| candidates.covers(info.codepoint))
    }

    #[inline]
    fn next_applicable(&self, ctx: &ApplyContext<'_>) -> usize {
        let (start, end) = (ctx.buffer.idx, ctx.buffer.len);
        ctx.buffer.info[start..end]
            .iter()
            .position(|info| self.applies_to(ctx, info))
            .map_or(end, |offset| start + offset)
    }
}

fn apply_forward(
    ctx: &mut ApplyContext<'_>,
    scan: &Scan<'_>,
    apply: &mut impl FnMut(&mut ApplyContext<'_>) -> bool,
) {
    while ctx.buffer.successful {
        let next = scan.next_applicable(ctx);
        if next > ctx.buffer.idx {
            let count = next - ctx.buffer.idx;
            ctx.buffer.next_glyphs(count);
        }
        if ctx.buffer.idx >= ctx.buffer.len {
            break;
        }
        if !apply(ctx) {
            ctx.buffer.next_glyph();
        }
    }
}

fn apply_backward(
    ctx: &mut ApplyContext<'_>,
    scan: &Scan<'_>,
    apply: &mut impl FnMut(&mut ApplyContext<'_>) -> bool,
) {
    loop {
        if scan.applies_to(ctx, &ctx.buffer.info[ctx.buffer.idx]) {
            apply(ctx);
        }
        if ctx.buffer.idx == 0 {
            break;
        }
        ctx.buffer.idx -= 1;
    }
}

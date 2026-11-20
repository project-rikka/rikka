use crate::buffer::glyph_buffer::Buffer;
use crate::buffer::scratch_flags;
use crate::face::font_context::FontContext;
use crate::plan::shape_plan::PlanCore;
use crate::unicode::ignorables::is_variation_selector;
use crate::unicode::space_fallback::SpaceKind;

const MAX_COMBINING_MARKS: usize = 32;

#[derive(Clone, Copy, Debug)]
pub(crate) struct NormalizeContext<'a> {
    pub(crate) plan: &'a PlanCore,
    pub(crate) font: &'a FontContext,
}

impl NormalizeContext<'_> {
    fn decompose(&self, codepoint: u32) -> Option<(u32, u32)> {
        self.plan.shaper.decompose(self, codepoint)
    }

    fn compose(&self, first: u32, second: u32) -> Option<u32> {
        self.plan.shaper.compose(self, first, second)
    }
}

fn output_char(buffer: &mut Buffer, codepoint: u32, glyph: u16) {
    buffer.cur_mut(0).nominal_glyph = glyph;
    buffer.output_glyph(codepoint);
    let mut scratch = buffer.scratch_flags;
    buffer.prev_mut().init_unicode_props(&mut scratch);
    buffer.scratch_flags = scratch;
}

fn next_char(buffer: &mut Buffer, glyph: u16) {
    buffer.cur_mut(0).nominal_glyph = glyph;
    buffer.next_glyph();
}

fn decompose(
    context: &NormalizeContext<'_>,
    buffer: &mut Buffer,
    shortest: bool,
    codepoint: u32,
) -> usize {
    let Some((first, second)) = context.decompose(codepoint) else {
        return 0;
    };
    let font = context.font;
    let first_glyph = font.nominal_glyph(first);
    let second_glyph = if second != 0 {
        match font.nominal_glyph(second) {
            Some(glyph) => Some(glyph),
            None => return 0,
        }
    } else {
        None
    };
    if shortest && let Some(first_glyph) = first_glyph {
        output_char(buffer, first, first_glyph);
        if let Some(second_glyph) = second_glyph {
            output_char(buffer, second, second_glyph);
            return 2;
        }
        return 1;
    }
    let deeper = decompose(context, buffer, shortest, first);
    if deeper != 0 {
        if let Some(second_glyph) = second_glyph {
            output_char(buffer, second, second_glyph);
            return deeper + 1;
        }
        return deeper;
    }
    if let Some(first_glyph) = first_glyph {
        output_char(buffer, first, first_glyph);
        if let Some(second_glyph) = second_glyph {
            output_char(buffer, second, second_glyph);
            return 2;
        }
        return 1;
    }
    0
}

fn decompose_current_character(
    context: &NormalizeContext<'_>,
    buffer: &mut Buffer,
    shortest: bool,
) {
    let codepoint = buffer.cur(0).codepoint;
    let font = context.font;
    let glyph = font.nominal_glyph(codepoint);
    if shortest && let Some(glyph) = glyph {
        next_char(buffer, glyph);
        return;
    }
    if decompose(context, buffer, shortest, codepoint) > 0 {
        buffer.skip_glyph();
        return;
    }
    if let Some(glyph) = glyph {
        next_char(buffer, glyph);
        return;
    }
    if buffer.cur(0).is_unicode_space() {
        let kind = SpaceKind::from_codepoint(codepoint);
        if kind != SpaceKind::NotSpace
            && let Some(space) = font.nominal_glyph(0x0020)
        {
            buffer.cur_mut(0).set_space_fallback(kind);
            next_char(buffer, space);
            buffer.scratch_flags |= scratch_flags::HAS_SPACE_FALLBACK;
            return;
        }
    }
    if codepoint == 0x2011
        && let Some(hyphen) = font.nominal_glyph(0x2010)
    {
        next_char(buffer, hyphen);
        return;
    }
    next_char(buffer, 0);
}

fn set_nominal_glyph(context: &NormalizeContext<'_>, buffer: &mut Buffer) {
    let codepoint = buffer.cur(0).codepoint;
    if let Some(glyph) = context.font.nominal_glyph(codepoint) {
        buffer.cur_mut(0).nominal_glyph = glyph;
    }
}

fn handle_variation_selector_cluster(
    context: &NormalizeContext<'_>,
    buffer: &mut Buffer,
    end: usize,
) {
    while buffer.idx + 1 < end && buffer.successful {
        if is_variation_selector(buffer.cur(1).codepoint) {
            let base = buffer.cur(0).codepoint;
            let selector = buffer.cur(1).codepoint;
            if let Some(glyph) = context.font.variant_glyph(base, selector) {
                buffer.cur_mut(0).nominal_glyph = glyph;
                buffer.replace_glyphs(2, &[base]);
            } else {
                set_nominal_glyph(context, buffer);
                buffer.next_glyph();
                buffer.cur_mut(0).mark_as_variation_selector();
                set_nominal_glyph(context, buffer);
                buffer.next_glyph();
            }
            while buffer.idx < end && is_variation_selector(buffer.cur(0).codepoint) {
                set_nominal_glyph(context, buffer);
                buffer.next_glyph();
            }
        } else {
            set_nominal_glyph(context, buffer);
            buffer.next_glyph();
        }
    }
    if buffer.idx < end {
        set_nominal_glyph(context, buffer);
        buffer.next_glyph();
    }
}

fn decompose_multi_char_cluster(
    context: &NormalizeContext<'_>,
    buffer: &mut Buffer,
    end: usize,
    shortest: bool,
) {
    if buffer.info[buffer.idx..end]
        .iter()
        .any(|info| is_variation_selector(info.codepoint))
    {
        handle_variation_selector_cluster(context, buffer, end);
        return;
    }
    while buffer.idx < end && buffer.successful {
        decompose_current_character(context, buffer, shortest);
    }
}

pub(crate) fn normalize(plan: &PlanCore, font: &FontContext, buffer: &mut Buffer) {
    if buffer.len == 0 {
        return;
    }
    let mode = plan.shaper.normalization_mode().resolved();
    let context = NormalizeContext { plan, font };
    let always_short_circuit = mode.always_short_circuits();
    let might_short_circuit = mode.might_short_circuit();
    let mut all_simple = true;

    buffer.clear_output();
    let count = buffer.len;
    buffer.idx = 0;
    loop {
        let mut end = buffer.idx + 1;
        while end < count && !buffer.info[end].is_unicode_mark() {
            end += 1;
        }
        if end < count {
            end -= 1;
        }
        if might_short_circuit {
            let simple = end - buffer.idx;
            let mut done = 0;
            while done < simple {
                let codepoint = buffer.cur(done).codepoint;
                let Some(glyph) = font.nominal_glyph(codepoint) else {
                    break;
                };
                buffer.cur_mut(done).nominal_glyph = glyph;
                done += 1;
            }
            buffer.next_glyphs(done);
        }
        while buffer.idx < end && buffer.successful {
            decompose_current_character(&context, buffer, might_short_circuit);
        }
        if buffer.idx == count || !buffer.successful {
            break;
        }
        all_simple = false;
        end = buffer.idx + 1;
        while end < count && buffer.info[end].is_unicode_mark() {
            end += 1;
        }
        decompose_multi_char_cluster(&context, buffer, end, always_short_circuit);
        if buffer.idx >= count || !buffer.successful {
            break;
        }
    }
    buffer.sync();

    if !all_simple {
        reorder_marks(plan, buffer);
    }
    if buffer.scratch_flags & scratch_flags::HAS_CGJ != 0 {
        unhide_ineffective_grapheme_joiners(buffer);
    }
    if !all_simple && buffer.successful && mode.recomposes() {
        recompose(&context, buffer);
    }
}

fn reorder_marks(plan: &PlanCore, buffer: &mut Buffer) {
    let count = buffer.len;
    let mut index = 0;
    while index < count {
        if buffer.info[index].modified_combining_class() == 0 {
            index += 1;
            continue;
        }
        let mut end = index + 1;
        while end < count && buffer.info[end].modified_combining_class() != 0 {
            end += 1;
        }
        if end - index <= MAX_COMBINING_MARKS {
            buffer.sort(index, end, |a, b| {
                a.modified_combining_class() > b.modified_combining_class()
            });
            plan.shaper.reorder_marks(plan, buffer, index, end);
        }
        index = end + 1;
    }
}

fn unhide_ineffective_grapheme_joiners(buffer: &mut Buffer) {
    for index in 1..buffer.len.saturating_sub(1) {
        if buffer.info[index].codepoint == 0x034F {
            let last = buffer.info[index - 1].modified_combining_class();
            let next = buffer.info[index + 1].modified_combining_class();
            if next == 0 || last <= next {
                buffer.info[index].unhide();
            }
        }
    }
}

fn recompose(context: &NormalizeContext<'_>, buffer: &mut Buffer) {
    let count = buffer.len;
    let mut starter = 0;
    buffer.clear_output();
    buffer.next_glyph();
    while buffer.idx < count && buffer.successful {
        let current = *buffer.cur(0);
        if current.is_unicode_mark()
            && (starter == buffer.out_len - 1
                || buffer.prev().modified_combining_class() < current.modified_combining_class())
        {
            let first = buffer.out_info()[starter].codepoint;
            if let Some(composed) = context.compose(first, current.codepoint)
                && let Some(glyph) = context.font.nominal_glyph(composed)
            {
                buffer.next_glyph();
                if !buffer.successful {
                    return;
                }
                buffer.merge_out_clusters(starter, buffer.out_len);
                buffer.out_len -= 1;
                let mut scratch = buffer.scratch_flags;
                let info = &mut buffer.out_info_mut()[starter];
                info.codepoint = composed;
                info.nominal_glyph = glyph;
                info.init_unicode_props(&mut scratch);
                buffer.scratch_flags = scratch;
                continue;
            }
        }
        buffer.next_glyph();
        if buffer.prev().modified_combining_class() == 0 {
            starter = buffer.out_len - 1;
        }
    }
    buffer.sync();
}

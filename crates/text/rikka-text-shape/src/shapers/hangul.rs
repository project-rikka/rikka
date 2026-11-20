use std::any::Any;

use rikka_text_font::Tag;

use crate::buffer::glyph_buffer::Buffer;
use crate::common::buffer_flags::BufferFlags;
use crate::face::font_context::FontContext;
use crate::plan::map_builder::feature_flags as flags;
use crate::plan::planner::Planner;
use crate::plan::shape_plan::PlanCore;
use crate::shapers::complex_shaper::{
    ComplexShaper, NormalizationMode, ShaperData, ZeroWidthMarks,
};

const LJMO: u8 = 1;
const VJMO: u8 = 2;
const TJMO: u8 = 3;

const L_BASE: u32 = 0x1100;
const V_BASE: u32 = 0x1161;
const T_BASE: u32 = 0x11A7;
const L_COUNT: u32 = 19;
const V_COUNT: u32 = 21;
const T_COUNT: u32 = 28;
const N_COUNT: u32 = V_COUNT * T_COUNT;
const S_COUNT: u32 = L_COUNT * N_COUNT;
const S_BASE: u32 = 0xAC00;
const DOTTED_CIRCLE: u32 = 0x25CC;

#[derive(Debug)]
pub(crate) struct HangulShaper;

pub(crate) static HANGUL_SHAPER: HangulShaper = HangulShaper;

#[derive(Debug)]
struct HangulPlan {
    masks: [u32; 4],
}

fn is_combining_l(codepoint: u32) -> bool {
    (L_BASE..L_BASE + L_COUNT).contains(&codepoint)
}

fn is_combining_v(codepoint: u32) -> bool {
    (V_BASE..V_BASE + V_COUNT).contains(&codepoint)
}

fn is_combining_t(codepoint: u32) -> bool {
    (T_BASE + 1..T_BASE + T_COUNT).contains(&codepoint)
}

fn is_combined_s(codepoint: u32) -> bool {
    (S_BASE..S_BASE + S_COUNT).contains(&codepoint)
}

fn is_l(codepoint: u32) -> bool {
    matches!(codepoint, 0x1100..=0x115F | 0xA960..=0xA97C)
}

fn is_v(codepoint: u32) -> bool {
    matches!(codepoint, 0x1160..=0x11A7 | 0xD7B0..=0xD7C6)
}

fn is_t(codepoint: u32) -> bool {
    matches!(codepoint, 0x11A8..=0x11FF | 0xD7CB..=0xD7FB)
}

fn is_tone_mark(codepoint: u32) -> bool {
    matches!(codepoint, 0x302E..=0x302F)
}

fn is_zero_width(font: &FontContext, codepoint: u32) -> bool {
    font.nominal_glyph(codepoint)
        .is_some_and(|glyph| font.advance(u32::from(glyph)) == 0)
}

impl ComplexShaper for HangulShaper {
    fn collect_features(&self, planner: &mut Planner<'_>) {
        for tag in [b"ljmo", b"vjmo", b"tjmo"] {
            planner.map.add_feature(Tag::new(tag), flags::NONE, 1);
        }
    }

    fn override_features(&self, planner: &mut Planner<'_>) {
        planner.map.disable_feature(Tag::new(b"calt"));
    }

    fn create_data(&self, plan: &PlanCore, _: &FontContext) -> Option<ShaperData> {
        let map = &plan.map;
        let data: Box<dyn Any + Send + Sync> = Box::new(HangulPlan {
            masks: [
                0,
                map.one_mask(Tag::new(b"ljmo")),
                map.one_mask(Tag::new(b"vjmo")),
                map.one_mask(Tag::new(b"tjmo")),
            ],
        });
        Some(data)
    }

    fn preprocess_text(&self, _: &PlanCore, font: &FontContext, buffer: &mut Buffer) {
        compose_syllables(font, buffer);
    }

    fn normalization_mode(&self) -> NormalizationMode {
        NormalizationMode::None
    }

    fn setup_masks(&self, plan: &PlanCore, _: &FontContext, buffer: &mut Buffer) {
        let Some(data) = plan.shaper_data::<HangulPlan>() else {
            return;
        };
        for info in &mut buffer.info[..buffer.len] {
            info.mask |= data.masks[usize::from(info.shaper_auxiliary.min(TJMO))];
        }
    }

    fn zero_width_marks(&self) -> ZeroWidthMarks {
        ZeroWidthMarks::None
    }
}

fn compose_syllables(font: &FontContext, buffer: &mut Buffer) {
    buffer.clear_output();
    let mut start = 0;
    let mut end = 0;
    buffer.idx = 0;
    while buffer.idx < buffer.len {
        let codepoint = buffer.cur(0).codepoint;
        if is_tone_mark(codepoint) {
            if start < end && end == buffer.out_len {
                let idx = buffer.idx;
                buffer.unsafe_to_break_from_outbuffer(start, idx);
                buffer.next_glyph();
                if !is_zero_width(font, codepoint) {
                    buffer.merge_out_clusters(start, end + 1);
                    let out = buffer.out_info_mut();
                    out[start..=end].rotate_right(1);
                }
            } else if !buffer
                .flags
                .contains(BufferFlags::DO_NOT_INSERT_DOTTED_CIRCLE)
                && font.has_glyph(DOTTED_CIRCLE)
            {
                let replacement = if is_zero_width(font, codepoint) {
                    [DOTTED_CIRCLE, codepoint]
                } else {
                    [codepoint, DOTTED_CIRCLE]
                };
                buffer.replace_glyphs(1, &replacement);
            } else {
                buffer.next_glyph();
            }
            start = buffer.out_len;
            end = buffer.out_len;
            continue;
        }
        start = buffer.out_len;
        if is_l(codepoint) && buffer.idx + 1 < buffer.len {
            let l = codepoint;
            let v = buffer.cur(1).codepoint;
            if is_v(v) {
                let mut t = 0;
                let mut t_index = 0;
                if buffer.idx + 2 < buffer.len {
                    t = buffer.cur(2).codepoint;
                    if is_t(t) {
                        t_index = t - T_BASE;
                    } else {
                        t = 0;
                    }
                }
                let length = if t != 0 { 3 } else { 2 };
                let idx = buffer.idx;
                buffer.unsafe_to_break(idx, idx + length);
                if is_combining_l(l) && is_combining_v(v) && (t == 0 || is_combining_t(t)) {
                    let syllable =
                        S_BASE + (l - L_BASE) * N_COUNT + (v - V_BASE) * T_COUNT + t_index;
                    if font.has_glyph(syllable) {
                        buffer.replace_glyphs(length, &[syllable]);
                        end = start + 1;
                        continue;
                    }
                }
                buffer.cur_mut(0).shaper_auxiliary = LJMO;
                buffer.next_glyph();
                buffer.cur_mut(0).shaper_auxiliary = VJMO;
                buffer.next_glyph();
                if t != 0 {
                    buffer.cur_mut(0).shaper_auxiliary = TJMO;
                    buffer.next_glyph();
                    end = start + 3;
                } else {
                    end = start + 2;
                }
                buffer.merge_out_clusters(start, end);
                continue;
            }
        } else if is_combined_s(codepoint) {
            let syllable = codepoint;
            let has_glyph = font.has_glyph(syllable);
            let l_index = (syllable - S_BASE) / N_COUNT;
            let n_index = (syllable - S_BASE) % N_COUNT;
            let v_index = n_index / T_COUNT;
            let t_index = n_index % T_COUNT;
            let next = (buffer.idx + 1 < buffer.len).then(|| buffer.cur(1).codepoint);
            if t_index == 0
                && let Some(next) = next.filter(|&next| is_combining_t(next))
            {
                let combined = syllable + (next - T_BASE);
                if font.has_glyph(combined) {
                    buffer.replace_glyphs(2, &[combined]);
                    end = start + 1;
                    continue;
                }
                let idx = buffer.idx;
                buffer.unsafe_to_break(idx, idx + 2);
            }
            if !has_glyph || (t_index == 0 && next.is_some_and(is_t)) {
                let decomposed = [L_BASE + l_index, V_BASE + v_index, T_BASE + t_index];
                if font.has_glyph(decomposed[0])
                    && font.has_glyph(decomposed[1])
                    && (t_index == 0 || font.has_glyph(decomposed[2]))
                {
                    let mut length = if t_index != 0 { 3 } else { 2 };
                    buffer.replace_glyphs(1, &decomposed[..length]);
                    if has_glyph && t_index == 0 {
                        buffer.next_glyph();
                        length += 1;
                    }
                    end = start + length;
                    let out = buffer.out_info_mut();
                    out[start].shaper_auxiliary = LJMO;
                    out[start + 1].shaper_auxiliary = VJMO;
                    if start + 2 < end {
                        out[start + 2].shaper_auxiliary = TJMO;
                    }
                    buffer.merge_out_clusters(start, end);
                    continue;
                }
            }
            if has_glyph {
                end = start + 1;
                buffer.next_glyph();
                continue;
            }
        }
        buffer.next_glyph();
    }
    buffer.sync();
}

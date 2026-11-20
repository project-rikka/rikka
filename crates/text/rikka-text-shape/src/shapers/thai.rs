use rikka_text_unicode::{GeneralCategory, Script};

use crate::buffer::glyph_buffer::Buffer;
use crate::face::font_context::FontContext;
use crate::plan::feature_map::TableKind;
use crate::plan::shape_plan::PlanCore;
use crate::shapers::complex_shaper::ComplexShaper;

#[derive(Debug)]
pub(crate) struct ThaiShaper;

pub(crate) static THAI_SHAPER: ThaiShaper = ThaiShaper;

impl ComplexShaper for ThaiShaper {
    fn preprocess_text(&self, plan: &PlanCore, font: &FontContext, buffer: &mut Buffer) {
        decompose_sara_am(buffer);
        if plan.properties.script == Script::Thai && !plan.map.found_script(TableKind::Gsub) {
            shape_with_private_use_forms(font, buffer);
        }
    }

    fn fallback_position(&self) -> bool {
        false
    }
}

fn is_sara_am(codepoint: u32) -> bool {
    codepoint & !0x0080 == 0x0E33
}

fn is_above_base_mark(codepoint: u32) -> bool {
    matches!(codepoint & !0x0080, 0x0E31 | 0x0E34..=0x0E37 | 0x0E3B | 0x0E47..=0x0E4E)
}

fn decompose_sara_am(buffer: &mut Buffer) {
    buffer.clear_output();
    buffer.idx = 0;
    while buffer.idx < buffer.len {
        let codepoint = buffer.cur(0).codepoint;
        if !is_sara_am(codepoint) {
            buffer.next_glyph();
            continue;
        }
        buffer.output_glyph(codepoint - 0x0E33 + 0x0E4D);
        let mut scratch = buffer.scratch_flags;
        buffer.prev_mut().set_continuation(&mut scratch);
        buffer.scratch_flags = scratch;
        buffer.replace_glyph(codepoint - 1);
        let end = buffer.out_len;
        buffer.out_info_mut()[end - 2].set_general_category(GeneralCategory::NonspacingMark);
        let mut start = end - 2;
        while start > 0 && is_above_base_mark(buffer.out_info()[start - 1].codepoint) {
            start -= 1;
        }
        if start + 2 < end {
            buffer.merge_out_clusters(start, end);
            buffer.out_info_mut()[start..end - 1].rotate_right(1);
        }
        if start != 0 {
            buffer.merge_out_clusters(start - 1, end);
        }
    }
    buffer.sync();
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Consonant {
    Normal,
    Ascender,
    RemovableDescender,
    StrictDescender,
    Other,
}

fn consonant_type(codepoint: u32) -> Consonant {
    match codepoint {
        0x0E1B | 0x0E1D | 0x0E1F => Consonant::Ascender,
        0x0E0D | 0x0E10 => Consonant::RemovableDescender,
        0x0E0E | 0x0E0F => Consonant::StrictDescender,
        0x0E01..=0x0E2E => Consonant::Normal,
        _ => Consonant::Other,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mark {
    AboveVowel,
    BelowVowel,
    Tone,
}

fn mark_type(codepoint: u32) -> Option<Mark> {
    match codepoint {
        0x0E31 | 0x0E34..=0x0E37 | 0x0E47 | 0x0E4D..=0x0E4E => Some(Mark::AboveVowel),
        0x0E38..=0x0E3A => Some(Mark::BelowVowel),
        0x0E48..=0x0E4C => Some(Mark::Tone),
        _ => None,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    None,
    ShiftDown,
    ShiftLeft,
    ShiftDownLeft,
    RemoveDescender,
}

const SHIFT_DOWN: [(u32, u32, u32); 8] = [
    (0x0E48, 0xF70A, 0xF88B),
    (0x0E49, 0xF70B, 0xF88E),
    (0x0E4A, 0xF70C, 0xF891),
    (0x0E4B, 0xF70D, 0xF894),
    (0x0E4C, 0xF70E, 0xF897),
    (0x0E38, 0xF718, 0xF89B),
    (0x0E39, 0xF719, 0xF89C),
    (0x0E3A, 0xF71A, 0xF89D),
];

const SHIFT_DOWN_LEFT: [(u32, u32, u32); 5] = [
    (0x0E48, 0xF705, 0xF88C),
    (0x0E49, 0xF706, 0xF88F),
    (0x0E4A, 0xF707, 0xF892),
    (0x0E4B, 0xF708, 0xF895),
    (0x0E4C, 0xF709, 0xF898),
];

const SHIFT_LEFT: [(u32, u32, u32); 12] = [
    (0x0E48, 0xF713, 0xF88A),
    (0x0E49, 0xF714, 0xF88D),
    (0x0E4A, 0xF715, 0xF890),
    (0x0E4B, 0xF716, 0xF893),
    (0x0E4C, 0xF717, 0xF896),
    (0x0E31, 0xF710, 0xF884),
    (0x0E34, 0xF701, 0xF885),
    (0x0E35, 0xF702, 0xF886),
    (0x0E36, 0xF703, 0xF887),
    (0x0E37, 0xF704, 0xF888),
    (0x0E47, 0xF712, 0xF889),
    (0x0E4D, 0xF711, 0xF899),
];

const REMOVE_DESCENDER: [(u32, u32, u32); 2] = [(0x0E0D, 0xF70F, 0xF89A), (0x0E10, 0xF700, 0xF89E)];

fn private_use_form(font: &FontContext, codepoint: u32, action: Action) -> u32 {
    let mappings: &[(u32, u32, u32)] = match action {
        Action::None => return codepoint,
        Action::ShiftDown => &SHIFT_DOWN,
        Action::ShiftLeft => &SHIFT_LEFT,
        Action::ShiftDownLeft => &SHIFT_DOWN_LEFT,
        Action::RemoveDescender => &REMOVE_DESCENDER,
    };
    let Some(&(_, windows, macintosh)) =
        mappings.iter().find(|(source, _, _)| *source == codepoint)
    else {
        return codepoint;
    };
    if font.has_glyph(windows) {
        windows
    } else if font.has_glyph(macintosh) {
        macintosh
    } else {
        codepoint
    }
}

#[derive(Clone, Copy)]
enum Above {
    T0,
    T1,
    T2,
    T3,
}

#[derive(Clone, Copy)]
enum Below {
    B0,
    B1,
    B2,
}

fn above_start(consonant: Consonant) -> Above {
    match consonant {
        Consonant::Ascender => Above::T1,
        Consonant::Other => Above::T3,
        _ => Above::T0,
    }
}

fn below_start(consonant: Consonant) -> Below {
    match consonant {
        Consonant::RemovableDescender => Below::B1,
        Consonant::StrictDescender | Consonant::Other => Below::B2,
        _ => Below::B0,
    }
}

fn above_edge(state: Above, mark: Mark) -> (Action, Above) {
    match (state, mark) {
        (Above::T0, Mark::AboveVowel) => (Action::None, Above::T3),
        (Above::T0, Mark::BelowVowel) => (Action::None, Above::T0),
        (Above::T0, Mark::Tone) => (Action::ShiftDown, Above::T3),
        (Above::T1, Mark::AboveVowel) => (Action::ShiftLeft, Above::T2),
        (Above::T1, Mark::BelowVowel) => (Action::None, Above::T1),
        (Above::T1, Mark::Tone) => (Action::ShiftDownLeft, Above::T2),
        (Above::T2, Mark::AboveVowel) => (Action::None, Above::T3),
        (Above::T2, Mark::BelowVowel) => (Action::None, Above::T2),
        (Above::T2, Mark::Tone) => (Action::ShiftLeft, Above::T3),
        (Above::T3, _) => (Action::None, Above::T3),
    }
}

fn below_edge(state: Below, mark: Mark) -> (Action, Below) {
    match (state, mark) {
        (Below::B0, Mark::BelowVowel) => (Action::None, Below::B2),
        (Below::B0, _) => (Action::None, Below::B0),
        (Below::B1, Mark::BelowVowel) => (Action::RemoveDescender, Below::B2),
        (Below::B1, _) => (Action::None, Below::B1),
        (Below::B2, Mark::BelowVowel) => (Action::ShiftDown, Below::B2),
        (Below::B2, _) => (Action::None, Below::B2),
    }
}

fn shape_with_private_use_forms(font: &FontContext, buffer: &mut Buffer) {
    let mut above = above_start(Consonant::Other);
    let mut below = below_start(Consonant::Other);
    let mut base = 0;
    for index in 0..buffer.len {
        let codepoint = buffer.info[index].codepoint;
        let Some(mark) = mark_type(codepoint) else {
            let consonant = consonant_type(codepoint);
            above = above_start(consonant);
            below = below_start(consonant);
            base = index;
            continue;
        };
        let (above_action, next_above) = above_edge(above, mark);
        let (below_action, next_below) = below_edge(below, mark);
        above = next_above;
        below = next_below;
        let action = if above_action != Action::None {
            above_action
        } else {
            below_action
        };
        buffer.unsafe_to_break(base, index);
        if action == Action::RemoveDescender {
            buffer.info[base].codepoint =
                private_use_form(font, buffer.info[base].codepoint, action);
        } else {
            buffer.info[index].codepoint = private_use_form(font, codepoint, action);
        }
    }
}

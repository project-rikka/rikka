use rikka_text_font::Tag;
use rikka_text_unicode::{JoiningGroup, JoiningType, Script};

use crate::buffer::glyph_buffer::Buffer;
use crate::plan::feature_map::FeatureMap;
use crate::unicode::to_char;

pub(crate) const JOINING_FEATURES: [Tag; 7] = [
    Tag::new(b"isol"),
    Tag::new(b"fina"),
    Tag::new(b"fin2"),
    Tag::new(b"fin3"),
    Tag::new(b"medi"),
    Tag::new(b"med2"),
    Tag::new(b"init"),
];

pub(crate) mod action {
    pub(crate) const ISOL: u8 = 0;
    pub(crate) const FINA: u8 = 1;
    pub(crate) const FIN2: u8 = 2;
    pub(crate) const FIN3: u8 = 3;
    pub(crate) const MEDI: u8 = 4;
    pub(crate) const MED2: u8 = 5;
    pub(crate) const INIT: u8 = 6;
    pub(crate) const NONE: u8 = 7;
    pub(crate) const STRETCHING_FIXED: u8 = 8;
    pub(crate) const STRETCHING_REPEATING: u8 = 9;

    #[inline]
    pub(crate) fn is_stretching(action: u8) -> bool {
        matches!(action, STRETCHING_FIXED | STRETCHING_REPEATING)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
enum Joining {
    Separate = 0,
    Left = 1,
    Right = 2,
    Dual = 3,
    Alaph = 4,
    DalathRish = 5,
    Transparent = 6,
}

fn joining_of(codepoint: u32) -> Joining {
    let character = to_char(codepoint);
    match rikka_text_unicode::joining_group(character) {
        JoiningGroup::Alaph => return Joining::Alaph,
        JoiningGroup::DalathRish => return Joining::DalathRish,
        _ => {}
    }
    match rikka_text_unicode::joining_type(character) {
        JoiningType::JoinCausing | JoiningType::DualJoining => Joining::Dual,
        JoiningType::LeftJoining => Joining::Left,
        JoiningType::RightJoining => Joining::Right,
        JoiningType::Transparent => Joining::Transparent,
        JoiningType::NonJoining => Joining::Separate,
    }
}

use action::{FIN2, FIN3, FINA, INIT, ISOL, MED2, MEDI, NONE};

static STATES: [[(u8, u8, u8); 6]; 7] = [
    [
        (NONE, NONE, 0),
        (NONE, ISOL, 2),
        (NONE, ISOL, 1),
        (NONE, ISOL, 2),
        (NONE, ISOL, 1),
        (NONE, ISOL, 6),
    ],
    [
        (NONE, NONE, 0),
        (NONE, ISOL, 2),
        (NONE, ISOL, 1),
        (NONE, ISOL, 2),
        (NONE, FIN2, 5),
        (NONE, ISOL, 6),
    ],
    [
        (NONE, NONE, 0),
        (NONE, ISOL, 2),
        (INIT, FINA, 1),
        (INIT, FINA, 3),
        (INIT, FINA, 4),
        (INIT, FINA, 6),
    ],
    [
        (NONE, NONE, 0),
        (NONE, ISOL, 2),
        (MEDI, FINA, 1),
        (MEDI, FINA, 3),
        (MEDI, FINA, 4),
        (MEDI, FINA, 6),
    ],
    [
        (NONE, NONE, 0),
        (NONE, ISOL, 2),
        (MED2, ISOL, 1),
        (MED2, ISOL, 2),
        (MED2, FIN2, 5),
        (MED2, ISOL, 6),
    ],
    [
        (NONE, NONE, 0),
        (NONE, ISOL, 2),
        (ISOL, ISOL, 1),
        (ISOL, ISOL, 2),
        (ISOL, FIN2, 5),
        (ISOL, ISOL, 6),
    ],
    [
        (NONE, NONE, 0),
        (NONE, ISOL, 2),
        (NONE, ISOL, 1),
        (NONE, ISOL, 2),
        (NONE, FIN3, 5),
        (NONE, ISOL, 6),
    ],
];

#[derive(Clone, Copy, Debug)]
pub(crate) struct JoiningPlan {
    masks: [u32; JOINING_FEATURES.len() + 1],
}

impl JoiningPlan {
    pub(crate) fn new(map: &FeatureMap) -> Self {
        let mut masks = [0; JOINING_FEATURES.len() + 1];
        for (mask, tag) in masks.iter_mut().zip(JOINING_FEATURES) {
            *mask = map.one_mask(tag);
        }
        Self { masks }
    }

    pub(crate) fn setup_masks(&self, buffer: &mut Buffer, script: Script) {
        resolve_joining(buffer);
        if script == Script::Mongolian {
            copy_actions_to_variation_selectors(buffer);
        }
        let masks = self.masks;
        for info in &mut buffer.info[..buffer.len] {
            info.mask |= masks
                .get(usize::from(info.shaper_auxiliary))
                .copied()
                .unwrap_or(0);
        }
    }
}

fn resolve_joining(buffer: &mut Buffer) {
    let mut previous: Option<usize> = None;
    let mut state = 0usize;
    for &codepoint in &buffer.context[0][..buffer.context_len[0]] {
        let joining = joining_of(codepoint);
        if joining == Joining::Transparent {
            continue;
        }
        state = usize::from(STATES[state][joining as usize].2);
        break;
    }
    for index in 0..buffer.len {
        let joining = joining_of(buffer.info[index].codepoint);
        if joining == Joining::Transparent {
            buffer.info[index].shaper_auxiliary = NONE;
            continue;
        }
        let (previous_action, current_action, next_state) = STATES[state][joining as usize];
        match previous {
            Some(previous_index) if previous_action != NONE => {
                buffer.info[previous_index].shaper_auxiliary = previous_action;
                buffer.safe_to_insert_tatweel(previous_index, index + 1);
            }
            Some(previous_index) => {
                if joining >= Joining::Right || (2..=5).contains(&state) {
                    buffer.unsafe_to_concat(previous_index, index + 1);
                }
            }
            None => {
                if joining >= Joining::Right {
                    buffer.unsafe_to_concat_from_outbuffer(0, index + 1);
                }
            }
        }
        buffer.info[index].shaper_auxiliary = current_action;
        previous = Some(index);
        state = usize::from(next_state);
    }
    for index in 0..buffer.context_len[1] {
        let joining = joining_of(buffer.context[1][index]);
        if joining == Joining::Transparent {
            continue;
        }
        let (previous_action, _, _) = STATES[state][joining as usize];
        if let Some(previous_index) = previous {
            let len = buffer.len;
            if previous_action != NONE {
                buffer.info[previous_index].shaper_auxiliary = previous_action;
                buffer.safe_to_insert_tatweel(previous_index, len);
            } else if (2..=5).contains(&state) {
                buffer.unsafe_to_concat(previous_index, len);
            }
        }
        break;
    }
}

fn copy_actions_to_variation_selectors(buffer: &mut Buffer) {
    for index in 1..buffer.len {
        if matches!(buffer.info[index].codepoint, 0x180B..=0x180D | 0x180F) {
            buffer.info[index].shaper_auxiliary = buffer.info[index - 1].shaper_auxiliary;
        }
    }
}

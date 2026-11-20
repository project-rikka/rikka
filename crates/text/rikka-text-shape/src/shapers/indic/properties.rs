use std::ops::RangeInclusive;
use std::sync::OnceLock;

use rikka_text_unicode::{IndicPositionalCategory as Ipc, IndicSyllabicCategory as Isc};

use crate::unicode::codepoint_table::CodepointTable;
use crate::unicode::to_char;

pub(crate) mod category {
    pub(crate) const X: u8 = 0;
    pub(crate) const C: u8 = 1;
    pub(crate) const V: u8 = 2;
    pub(crate) const N: u8 = 3;
    pub(crate) const H: u8 = 4;
    pub(crate) const ZWNJ: u8 = 5;
    pub(crate) const ZWJ: u8 = 6;
    pub(crate) const M: u8 = 7;
    pub(crate) const SM: u8 = 8;
    pub(crate) const A: u8 = 9;
    pub(crate) const PLACEHOLDER: u8 = 10;
    pub(crate) const DOTTED_CIRCLE: u8 = 11;
    pub(crate) const RS: u8 = 12;
    pub(crate) const MPST: u8 = 13;
    pub(crate) const REPHA: u8 = 14;
    pub(crate) const RA: u8 = 15;
    pub(crate) const CM: u8 = 16;
    pub(crate) const SYMBOL: u8 = 17;
    pub(crate) const CS: u8 = 18;
    pub(crate) const VABV: u8 = 20;
    pub(crate) const VBLW: u8 = 21;
    pub(crate) const VPRE: u8 = 22;
    pub(crate) const VPST: u8 = 23;
    pub(crate) const ROBATIC: u8 = 25;
    pub(crate) const XGROUP: u8 = 26;
    pub(crate) const YGROUP: u8 = 27;
    pub(crate) const AS: u8 = 32;
    pub(crate) const MH: u8 = 35;
    pub(crate) const MR: u8 = 36;
    pub(crate) const MW: u8 = 37;
    pub(crate) const MY: u8 = 38;
    pub(crate) const PT: u8 = 39;
    pub(crate) const VS: u8 = 40;
    pub(crate) const ML: u8 = 41;
    pub(crate) const SMPST: u8 = 57;
}

pub(crate) mod position {
    pub(crate) const START: u8 = 0;
    pub(crate) const RA_TO_BECOME_REPH: u8 = 1;
    pub(crate) const PRE_M: u8 = 2;
    pub(crate) const PRE_C: u8 = 3;
    pub(crate) const BASE_C: u8 = 4;
    pub(crate) const AFTER_MAIN: u8 = 5;
    pub(crate) const ABOVE_C: u8 = 6;
    pub(crate) const BEFORE_SUB: u8 = 7;
    pub(crate) const BELOW_C: u8 = 8;
    pub(crate) const AFTER_SUB: u8 = 9;
    pub(crate) const POST_C: u8 = 11;
    pub(crate) const AFTER_POST: u8 = 12;
    pub(crate) const SMVD: u8 = 13;
    pub(crate) const END: u8 = 14;
}

use category as cat;
use position as pos;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Block {
    Devanagari,
    Bengali,
    Gurmukhi,
    Gujarati,
    Oriya,
    Tamil,
    Telugu,
    Kannada,
    Malayalam,
    Khmer,
    Myanmar,
    Other,
}

const BLOCKS: [(RangeInclusive<u32>, Block); 20] = [
    (0x0000..=0x007F, Block::Other),
    (0x0080..=0x00FF, Block::Other),
    (0x0900..=0x097F, Block::Devanagari),
    (0x0980..=0x09FF, Block::Bengali),
    (0x0A00..=0x0A7F, Block::Gurmukhi),
    (0x0A80..=0x0AFF, Block::Gujarati),
    (0x0B00..=0x0B7F, Block::Oriya),
    (0x0B80..=0x0BFF, Block::Tamil),
    (0x0C00..=0x0C7F, Block::Telugu),
    (0x0C80..=0x0CFF, Block::Kannada),
    (0x0D00..=0x0D7F, Block::Malayalam),
    (0x1000..=0x109F, Block::Myanmar),
    (0x1780..=0x17FF, Block::Khmer),
    (0x1CD0..=0x1CFF, Block::Other),
    (0x2000..=0x206F, Block::Other),
    (0x2070..=0x209F, Block::Other),
    (0xA8E0..=0xA8FF, Block::Other),
    (0xA9E0..=0xA9FF, Block::Myanmar),
    (0xAA60..=0xAA7F, Block::Myanmar),
    (0x116D0..=0x116FF, Block::Myanmar),
];

const SINGLES: [u32; 2] = [0x00A0, 0x25CC];

const CATEGORY_OVERRIDES: [(RangeInclusive<u32>, u8); 64] = [
    (0xFE00..=0xFE0F, cat::VS),
    (0x2015..=0x2015, cat::PLACEHOLDER),
    (0x2022..=0x2022, cat::PLACEHOLDER),
    (0x25FB..=0x25FE, cat::PLACEHOLDER),
    (0x0930..=0x0930, cat::RA),
    (0x09B0..=0x09B0, cat::RA),
    (0x09F0..=0x09F0, cat::RA),
    (0x0A30..=0x0A30, cat::RA),
    (0x0AB0..=0x0AB0, cat::RA),
    (0x0B30..=0x0B30, cat::RA),
    (0x0BB0..=0x0BB0, cat::RA),
    (0x0C30..=0x0C30, cat::RA),
    (0x0CB0..=0x0CB0, cat::RA),
    (0x0D30..=0x0D30, cat::RA),
    (0x0953..=0x0954, cat::SM),
    (0x0A40..=0x0A40, cat::MPST),
    (0x0A72..=0x0A73, cat::C),
    (0x1CE2..=0x1CE8, cat::A),
    (0x1CED..=0x1CED, cat::A),
    (0xA8F2..=0xA8F7, cat::SYMBOL),
    (0x1CE9..=0x1CEC, cat::SYMBOL),
    (0x1CEE..=0x1CF1, cat::SYMBOL),
    (0x1CF5..=0x1CF6, cat::C),
    (0x0A51..=0x0A51, cat::M),
    (0x11301..=0x11303, cat::SM),
    (0x1133B..=0x1133C, cat::N),
    (0x0AFB..=0x0AFB, cat::N),
    (0x0B55..=0x0B55, cat::N),
    (0x09FC..=0x09FC, cat::PLACEHOLDER),
    (0x0C80..=0x0C80, cat::PLACEHOLDER),
    (0x0D04..=0x0D04, cat::PLACEHOLDER),
    (0x25CC..=0x25CC, cat::DOTTED_CIRCLE),
    (0x179A..=0x179A, cat::RA),
    (0x17CC..=0x17CC, cat::ROBATIC),
    (0x17C9..=0x17CA, cat::ROBATIC),
    (0x17C6..=0x17C6, cat::XGROUP),
    (0x17CB..=0x17CB, cat::XGROUP),
    (0x17CD..=0x17D1, cat::XGROUP),
    (0x17C7..=0x17C8, cat::YGROUP),
    (0x17DD..=0x17DD, cat::YGROUP),
    (0x17D3..=0x17D3, cat::YGROUP),
    (0x17D9..=0x17D9, cat::PLACEHOLDER),
    (0x104E..=0x104E, cat::C),
    (0x1004..=0x1004, cat::RA),
    (0x101B..=0x101B, cat::RA),
    (0x105A..=0x105A, cat::RA),
    (0x1032..=0x1032, cat::A),
    (0x1036..=0x1036, cat::A),
    (0x103A..=0x103A, cat::AS),
    (0x103E..=0x103E, cat::MH),
    (0x1060..=0x1060, cat::ML),
    (0x103C..=0x103C, cat::MR),
    (0x103D..=0x103D, cat::MW),
    (0x1082..=0x1082, cat::MW),
    (0x103B..=0x103B, cat::MY),
    (0x105E..=0x105F, cat::MY),
    (0x1063..=0x1064, cat::PT),
    (0x1069..=0x106D, cat::PT),
    (0xAA7B..=0xAA7B, cat::PT),
    (0x1038..=0x1038, cat::SM),
    (0x1087..=0x108D, cat::SM),
    (0x108F..=0x108F, cat::SM),
    (0x109A..=0x109C, cat::SM),
    (0x104A..=0x104A, cat::PLACEHOLDER),
];

const POSITION_OVERRIDES: [(u32, u8); 2] = [(0x0A51, pos::BELOW_C), (0x0B01, pos::BEFORE_SUB)];

fn block_of(codepoint: u32) -> Option<Block> {
    BLOCKS
        .iter()
        .find(|(range, _)| range.contains(&codepoint))
        .map(|(_, block)| *block)
}

fn category_of(syllabic: Isc) -> u8 {
    match syllabic {
        Isc::Avagraha => cat::SYMBOL,
        Isc::Bindu | Isc::GeminationMark | Isc::SyllableModifier | Isc::Visarga => cat::SM,
        Isc::BrahmiJoiningNumber | Isc::ConsonantPlaceholder | Isc::Number | Isc::NumberJoiner => {
            cat::PLACEHOLDER
        }
        Isc::CantillationMark => cat::A,
        Isc::Consonant
        | Isc::ConsonantDead
        | Isc::ConsonantHeadLetter
        | Isc::ConsonantInitialPostfixed => cat::C,
        Isc::ConsonantFinal
        | Isc::ConsonantMedial
        | Isc::ConsonantSubjoined
        | Isc::ConsonantSucceedingRepha => cat::CM,
        Isc::ConsonantKiller | Isc::PureKiller | Isc::VowelDependent => cat::M,
        Isc::ConsonantPrecedingRepha => cat::REPHA,
        Isc::ConsonantWithStacker => cat::CS,
        Isc::InvisibleStacker | Isc::Virama => cat::H,
        Isc::Joiner => cat::ZWJ,
        Isc::NonJoiner => cat::ZWNJ,
        Isc::Nukta | Isc::ToneMark => cat::N,
        Isc::RegisterShifter => cat::RS,
        Isc::Vowel | Isc::VowelIndependent => cat::V,
        Isc::ConsonantPrefixed
        | Isc::ModifyingLetter
        | Isc::Other
        | Isc::ReorderingKiller
        | Isc::ToneLetter => cat::X,
    }
}

fn position_of(positional: Ipc) -> u8 {
    match positional {
        Ipc::NotApplicable => pos::END,
        Ipc::Left => pos::PRE_C,
        Ipc::Top | Ipc::TopAndLeft => pos::ABOVE_C,
        Ipc::Bottom | Ipc::BottomAndLeft | Ipc::TopAndBottom | Ipc::TopAndBottomAndLeft => {
            pos::BELOW_C
        }
        Ipc::Right
        | Ipc::BottomAndRight
        | Ipc::LeftAndRight
        | Ipc::TopAndBottomAndRight
        | Ipc::TopAndLeftAndRight
        | Ipc::TopAndRight => pos::POST_C,
        Ipc::Overstruck => pos::AFTER_MAIN,
        Ipc::VisualOrderLeft => pos::PRE_M,
    }
}

fn right_matra(codepoint: u32, block: Block) -> u8 {
    match block {
        Block::Devanagari => pos::AFTER_SUB,
        Block::Bengali
        | Block::Gurmukhi
        | Block::Gujarati
        | Block::Oriya
        | Block::Tamil
        | Block::Malayalam => pos::AFTER_POST,
        Block::Telugu if codepoint <= 0x0C42 => pos::BEFORE_SUB,
        Block::Kannada if !(0x0CC3..=0x0CD6).contains(&codepoint) => pos::BEFORE_SUB,
        _ => pos::AFTER_SUB,
    }
}

fn top_matra(block: Block) -> u8 {
    match block {
        Block::Gurmukhi => pos::AFTER_POST,
        Block::Oriya => pos::AFTER_MAIN,
        Block::Telugu | Block::Kannada => pos::BEFORE_SUB,
        _ => pos::AFTER_SUB,
    }
}

fn bottom_matra(block: Block) -> u8 {
    match block {
        Block::Gurmukhi | Block::Gujarati | Block::Tamil | Block::Malayalam => pos::AFTER_POST,
        Block::Telugu | Block::Kannada => pos::BEFORE_SUB,
        _ => pos::AFTER_SUB,
    }
}

fn matra_position(codepoint: u32, position: u8, block: Block) -> u8 {
    match position {
        pos::PRE_C => pos::PRE_M,
        pos::POST_C => right_matra(codepoint, block),
        pos::ABOVE_C => top_matra(block),
        pos::BELOW_C => bottom_matra(block),
        other => other,
    }
}

fn split_vowel_category(position: u8) -> u8 {
    match position {
        pos::PRE_C => cat::VPRE,
        pos::ABOVE_C => cat::VABV,
        pos::BELOW_C => cat::VBLW,
        _ => cat::VPST,
    }
}

fn derive(codepoint: u32) -> (u8, u8) {
    let block = block_of(codepoint);
    let listed = block.is_some() || SINGLES.contains(&codepoint);
    let (mut category, mut position) = if listed {
        let character = to_char(codepoint);
        let category = category_of(rikka_text_unicode::indic_syllabic_category(character));
        let position = position_of(rikka_text_unicode::indic_positional_category(character));
        if category == cat::SM && position == pos::END {
            (cat::SMPST, position)
        } else {
            (category, position)
        }
    } else {
        (cat::X, pos::END)
    };
    if let Some((_, overridden)) = CATEGORY_OVERRIDES
        .iter()
        .find(|(range, _)| range.contains(&codepoint))
    {
        category = *overridden;
    }
    if !matches!(
        category,
        cat::CM | cat::SM | cat::RS | cat::H | cat::M | cat::MPST
    ) {
        position = pos::END;
    }
    let block = block.unwrap_or(Block::Other);
    match category {
        cat::C | cat::CS | cat::RA | cat::CM | cat::V | cat::PLACEHOLDER | cat::DOTTED_CIRCLE => {
            position = pos::BASE_C;
        }
        cat::M | cat::MPST if matches!(block, Block::Khmer | Block::Myanmar) => {
            category = split_vowel_category(position);
        }
        cat::M | cat::MPST => position = matra_position(codepoint, position, block),
        cat::SM | cat::SMPST | cat::A | cat::SYMBOL => position = pos::SMVD,
        _ => {}
    }
    if let Some((_, overridden)) = POSITION_OVERRIDES
        .iter()
        .find(|(overridden, _)| *overridden == codepoint)
    {
        position = *overridden;
    }
    (category, position)
}

fn table() -> &'static CodepointTable {
    static TABLE: OnceLock<CodepointTable> = OnceLock::new();
    TABLE.get_or_init(|| {
        let ranges = BLOCKS
            .iter()
            .map(|(range, _)| range.clone())
            .chain(SINGLES.iter().map(|&single| single..=single))
            .chain(CATEGORY_OVERRIDES.iter().map(|(range, _)| range.clone()));
        CodepointTable::new(
            ranges,
            u16::from(cat::X) | u16::from(pos::END) << 8,
            |codepoint| {
                let (category, position) = derive(codepoint);
                u16::from(category) | u16::from(position) << 8
            },
        )
    })
}

#[inline]
pub(crate) fn indic_properties(codepoint: u32) -> (u8, u8) {
    let value = table().get(codepoint);
    (value as u8, (value >> 8) as u8)
}

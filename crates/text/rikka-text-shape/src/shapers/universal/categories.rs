use std::sync::OnceLock;

use rikka_text_unicode::{
    GeneralCategory as Gc, IndicPositionalCategory as Ipc, IndicSyllabicCategory as Isc,
    JoiningType, Script,
};

use crate::shapers::universal::overrides::{POSITIONAL_OVERRIDES, SYLLABIC_OVERRIDES};
use crate::shapers::universal::syllabic::Syllabic;
use crate::unicode::codepoint_table::CodepointTable;
use crate::unicode::to_char;

pub(crate) mod category {
    pub(crate) const O: u8 = 0;
    pub(crate) const B: u8 = 1;
    pub(crate) const N: u8 = 4;
    pub(crate) const GB: u8 = 5;
    pub(crate) const CGJ: u8 = 6;
    pub(crate) const SUB: u8 = 11;
    pub(crate) const H: u8 = 12;
    pub(crate) const HN: u8 = 13;
    pub(crate) const ZWNJ: u8 = 14;
    pub(crate) const WJ: u8 = 16;
    pub(crate) const R: u8 = 18;
    pub(crate) const VPRE: u8 = 22;
    pub(crate) const VMPRE: u8 = 23;
    pub(crate) const FABV: u8 = 24;
    pub(crate) const FBLW: u8 = 25;
    pub(crate) const FPST: u8 = 26;
    pub(crate) const MABV: u8 = 27;
    pub(crate) const MBLW: u8 = 28;
    pub(crate) const MPST: u8 = 29;
    pub(crate) const MPRE: u8 = 30;
    pub(crate) const CMABV: u8 = 31;
    pub(crate) const CMBLW: u8 = 32;
    pub(crate) const VABV: u8 = 33;
    pub(crate) const VBLW: u8 = 34;
    pub(crate) const VPST: u8 = 35;
    pub(crate) const VMABV: u8 = 37;
    pub(crate) const VMBLW: u8 = 38;
    pub(crate) const VMPST: u8 = 39;
    pub(crate) const SMABV: u8 = 41;
    pub(crate) const SMBLW: u8 = 42;
    pub(crate) const CS: u8 = 43;
    pub(crate) const IS: u8 = 44;
    pub(crate) const FMABV: u8 = 45;
    pub(crate) const FMBLW: u8 = 46;
    pub(crate) const FMPST: u8 = 47;
    pub(crate) const SK: u8 = 48;
    pub(crate) const G: u8 = 49;
    pub(crate) const J: u8 = 50;
    pub(crate) const SB: u8 = 51;
    pub(crate) const SE: u8 = 52;
    pub(crate) const HVM: u8 = 53;
    pub(crate) const HM: u8 = 54;
    pub(crate) const HR: u8 = 55;
    pub(crate) const RK: u8 = 56;
}

use category as cat;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    Base,
    BaseNumber,
    BaseOther,
    Cgj,
    ConsonantFinal,
    ConsonantFinalModifier,
    ConsonantMedial,
    ConsonantModifier,
    ConsonantSubjoined,
    ConsonantWithStacker,
    Halant,
    HalantOrVowelModifier,
    HalantNumber,
    Hieroglyph,
    HieroglyphJoiner,
    HieroglyphMirror,
    HieroglyphModifier,
    HieroglyphSegmentBegin,
    HieroglyphSegmentEnd,
    InvisibleStacker,
    NonJoiner,
    Other,
    ReorderingKiller,
    Repha,
    Sakot,
    SymbolModifier,
    Vowel,
    VowelModifier,
    WordJoiner,
}

const DISABLED_SCRIPTS: [Script; 5] = [
    Script::Arabic,
    Script::Lao,
    Script::Samaritan,
    Script::Syriac,
    Script::Thai,
];

const HALANT_OR_VOWEL_MODIFIER: u32 = 0x0DCA;
const SAKOT: u32 = 0x1A60;
const NOT_WORD_JOINERS: [u32; 8] = [
    0x115F, 0x1160, 0x3164, 0xFFA0, 0x1BCA0, 0x1BCA1, 0x1BCA2, 0x1BCA3,
];

struct Overrides {
    syllabic: Vec<(u32, u32, Syllabic)>,
    positional: Vec<(u32, u32, Ipc)>,
}

fn sorted<T: Copy>(entries: &[(u32, u32, T)]) -> Vec<(u32, u32, T)> {
    let mut sorted = entries.to_vec();
    sorted.sort_by_key(|(first, _, _)| *first);
    sorted
}

fn find<T: Copy>(entries: &[(u32, u32, T)], codepoint: u32) -> Option<T> {
    let index = entries.partition_point(|(first, _, _)| *first <= codepoint);
    let (_, last, value) = *entries.get(index.checked_sub(1)?)?;
    (codepoint <= last).then_some(value)
}

fn syllabic_of(overrides: &Overrides, codepoint: u32) -> Syllabic {
    let syllabic = find(&overrides.syllabic, codepoint).unwrap_or_else(|| {
        Syllabic::Unicode(rikka_text_unicode::indic_syllabic_category(to_char(
            codepoint,
        )))
    });
    match codepoint {
        0x1CE2..=0x1CE8 => Syllabic::Unicode(Isc::CantillationMark),
        0x0F18..=0x0F19 | 0x0F3E..=0x0F3F => Syllabic::Unicode(Isc::VowelDependent),
        0x1CED => Syllabic::Unicode(Isc::ToneMark),
        _ => syllabic,
    }
}

fn positional_of(overrides: &Overrides, codepoint: u32) -> Ipc {
    match codepoint {
        0x11302 | 0x11303 | 0x114C1 => Ipc::Top,
        _ => find(&overrides.positional, codepoint)
            .unwrap_or_else(|| rikka_text_unicode::indic_positional_category(to_char(codepoint))),
    }
}

struct Traits {
    codepoint: u32,
    syllabic: Syllabic,
    ignorable: bool,
    general: Gc,
    joining: bool,
}

impl Traits {
    fn is(&self, categories: &[Isc]) -> bool {
        matches!(self.syllabic, Syllabic::Unicode(isc) if categories.contains(&isc))
    }

    fn is_base(&self) -> bool {
        self.is(&[
            Isc::Number,
            Isc::Consonant,
            Isc::ConsonantHeadLetter,
            Isc::ToneLetter,
            Isc::VowelIndependent,
        ]) || (self.joining && !self.is(&[Isc::Joiner]))
            || (self.general == Gc::OtherLetter
                && self.is(&[
                    Isc::Avagraha,
                    Isc::Bindu,
                    Isc::ConsonantFinal,
                    Isc::ConsonantMedial,
                    Isc::ConsonantSubjoined,
                    Isc::Vowel,
                    Isc::VowelDependent,
                ]))
    }

    fn is_base_other(&self) -> bool {
        self.is(&[Isc::ConsonantPlaceholder])
    }

    fn is_cgj(&self) -> bool {
        self.is(&[Isc::Joiner])
            || (self.ignorable
                && matches!(
                    self.general,
                    Gc::SpacingMark | Gc::EnclosingMark | Gc::NonspacingMark
                ))
    }

    fn is_word_joiner(&self) -> bool {
        (self.ignorable
            && !NOT_WORD_JOINERS.contains(&self.codepoint)
            && self.is(&[Isc::Other])
            && !self.is_cgj())
            || self.general == Gc::Unassigned
    }

    fn is_other(&self) -> bool {
        (self.general == Gc::OtherPunctuation
            || self.is(&[
                Isc::ConsonantDead,
                Isc::Joiner,
                Isc::ModifyingLetter,
                Isc::Other,
            ]))
            && !self.is_base()
            && !self.is_base_other()
            && !self.is_cgj()
            && self.syllabic != Syllabic::SymbolModifier
            && !self.is_word_joiner()
    }

    fn class(&self) -> Option<Class> {
        let not_letter = self.general != Gc::OtherLetter;
        let class = if self.is_base() {
            Class::Base
        } else if self.is(&[Isc::BrahmiJoiningNumber]) {
            Class::BaseNumber
        } else if self.is_base_other() {
            Class::BaseOther
        } else if self.is_cgj() {
            Class::Cgj
        } else if (self.is(&[Isc::ConsonantFinal]) && not_letter)
            || self.is(&[Isc::ConsonantSucceedingRepha])
        {
            Class::ConsonantFinal
        } else if self.is(&[Isc::SyllableModifier]) {
            Class::ConsonantFinalModifier
        } else if (self.is(&[Isc::ConsonantMedial]) && not_letter)
            || self.is(&[Isc::ConsonantInitialPostfixed])
        {
            Class::ConsonantMedial
        } else if self.is(&[Isc::Nukta, Isc::GeminationMark, Isc::ConsonantKiller]) {
            Class::ConsonantModifier
        } else if self.is(&[Isc::ConsonantSubjoined]) && not_letter {
            Class::ConsonantSubjoined
        } else if self.is(&[Isc::ConsonantWithStacker]) {
            Class::ConsonantWithStacker
        } else if self.codepoint == HALANT_OR_VOWEL_MODIFIER {
            Class::HalantOrVowelModifier
        } else if self.is(&[Isc::Virama]) {
            Class::Halant
        } else if self.is(&[Isc::NumberJoiner]) {
            Class::HalantNumber
        } else if self.codepoint == SAKOT {
            Class::Sakot
        } else if self.is(&[Isc::InvisibleStacker]) {
            Class::InvisibleStacker
        } else if self.is(&[Isc::NonJoiner]) {
            Class::NonJoiner
        } else if self.is(&[Isc::ReorderingKiller]) {
            Class::ReorderingKiller
        } else if self.is(&[Isc::ConsonantPrecedingRepha, Isc::ConsonantPrefixed]) {
            Class::Repha
        } else if self.is(&[Isc::PureKiller])
            || (not_letter && self.is(&[Isc::Vowel, Isc::VowelDependent]))
        {
            Class::Vowel
        } else if self.is(&[
            Isc::ToneMark,
            Isc::CantillationMark,
            Isc::RegisterShifter,
            Isc::Visarga,
        ]) || (not_letter && self.is(&[Isc::Bindu]))
        {
            Class::VowelModifier
        } else if self.is_other() {
            Class::Other
        } else if self.is_word_joiner() {
            Class::WordJoiner
        } else {
            match self.syllabic {
                Syllabic::SymbolModifier => Class::SymbolModifier,
                Syllabic::Hieroglyph => Class::Hieroglyph,
                Syllabic::HieroglyphJoiner => Class::HieroglyphJoiner,
                Syllabic::HieroglyphMirror => Class::HieroglyphMirror,
                Syllabic::HieroglyphModifier => Class::HieroglyphModifier,
                Syllabic::HieroglyphMarkBegin | Syllabic::HieroglyphSegmentBegin => {
                    Class::HieroglyphSegmentBegin
                }
                Syllabic::HieroglyphMarkEnd | Syllabic::HieroglyphSegmentEnd => {
                    Class::HieroglyphSegmentEnd
                }
                Syllabic::Unicode(_) => return None,
            }
        };
        Some(class)
    }
}

fn placed(positional: Ipc, above: u8, below: u8, post: u8, pre: u8) -> u8 {
    match positional {
        Ipc::Top => above,
        Ipc::Bottom => below,
        Ipc::Right => post,
        Ipc::Left => pre,
        _ => cat::O,
    }
}

fn category_of(class: Class, positional: Ipc) -> u8 {
    match class {
        Class::Base => cat::B,
        Class::BaseNumber => cat::N,
        Class::BaseOther => cat::GB,
        Class::Cgj => cat::CGJ,
        Class::ConsonantFinal => placed(positional, cat::FABV, cat::FBLW, cat::FPST, cat::O),
        Class::ConsonantFinalModifier => match positional {
            Ipc::Top => cat::FMABV,
            Ipc::Bottom => cat::FMBLW,
            Ipc::NotApplicable => cat::FMPST,
            _ => cat::O,
        },
        Class::ConsonantMedial => match positional {
            Ipc::Bottom | Ipc::BottomAndLeft | Ipc::BottomAndRight => cat::MBLW,
            Ipc::TopAndBottomAndLeft => cat::MPRE,
            other => placed(other, cat::MABV, cat::MBLW, cat::MPST, cat::MPRE),
        },
        Class::ConsonantModifier => match positional {
            Ipc::Top => cat::CMABV,
            Ipc::Bottom | Ipc::Overstruck => cat::CMBLW,
            _ => cat::O,
        },
        Class::ConsonantSubjoined => cat::SUB,
        Class::ConsonantWithStacker => cat::CS,
        Class::Halant => cat::H,
        Class::HalantOrVowelModifier => cat::HVM,
        Class::HalantNumber => cat::HN,
        Class::Hieroglyph => cat::G,
        Class::HieroglyphJoiner => cat::J,
        Class::HieroglyphMirror => cat::HR,
        Class::HieroglyphModifier => cat::HM,
        Class::HieroglyphSegmentBegin => cat::SB,
        Class::HieroglyphSegmentEnd => cat::SE,
        Class::InvisibleStacker => cat::IS,
        Class::NonJoiner => cat::ZWNJ,
        Class::Other => cat::O,
        Class::ReorderingKiller => cat::RK,
        Class::Repha => cat::R,
        Class::Sakot => cat::SK,
        Class::SymbolModifier => match positional {
            Ipc::Top => cat::SMABV,
            Ipc::Bottom => cat::SMBLW,
            _ => cat::O,
        },
        Class::Vowel => match positional {
            Ipc::TopAndBottom | Ipc::TopAndBottomAndRight | Ipc::TopAndRight => cat::VABV,
            Ipc::Overstruck | Ipc::BottomAndRight => cat::VBLW,
            Ipc::TopAndLeft | Ipc::TopAndLeftAndRight | Ipc::LeftAndRight => cat::VPRE,
            other => placed(other, cat::VABV, cat::VBLW, cat::VPST, cat::VPRE),
        },
        Class::VowelModifier => match positional {
            Ipc::Overstruck => cat::VMBLW,
            other => placed(other, cat::VMABV, cat::VMBLW, cat::VMPST, cat::VMPRE),
        },
        Class::WordJoiner => cat::WJ,
    }
}

fn derive(overrides: &Overrides, codepoint: u32) -> u8 {
    let character = to_char(codepoint);
    let properties = rikka_text_unicode::properties(character);
    let general = properties.general_category();
    let ignorable = properties.is_default_ignorable();
    if DISABLED_SCRIPTS.contains(&properties.script()) || (general == Gc::Unassigned && !ignorable)
    {
        return cat::O;
    }
    let traits = Traits {
        codepoint,
        syllabic: syllabic_of(overrides, codepoint),
        ignorable,
        general,
        joining: matches!(
            properties.joining_type(),
            JoiningType::JoinCausing
                | JoiningType::DualJoining
                | JoiningType::LeftJoining
                | JoiningType::RightJoining
        ),
    };
    traits.class().map_or(cat::O, |class| {
        category_of(class, positional_of(overrides, codepoint))
    })
}

fn table() -> &'static CodepointTable {
    static TABLE: OnceLock<CodepointTable> = OnceLock::new();
    TABLE.get_or_init(|| {
        let overrides = Overrides {
            syllabic: sorted(SYLLABIC_OVERRIDES),
            positional: sorted(POSITIONAL_OVERRIDES),
        };
        CodepointTable::new(
            [0..=0x1FFFF, 0xE0000..=0xE0FFF],
            u16::from(cat::O),
            move |codepoint| u16::from(derive(&overrides, codepoint)),
        )
    })
}

#[inline]
pub(crate) fn use_category(codepoint: u32) -> u8 {
    table().get(codepoint) as u8
}

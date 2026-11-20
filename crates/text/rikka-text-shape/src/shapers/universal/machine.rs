use std::sync::OnceLock;

use crate::buffer::glyph_buffer::Buffer;
use crate::buffer::glyph_info::GlyphInfo;
use crate::shapers::syllabic::machine::SyllableMachine;
use crate::shapers::syllabic::pattern::{Pattern, any, any_of, either, one, opt, plus, seq, star};
use crate::shapers::syllabic::syllables::find_syllables_skipping;
use crate::shapers::universal::categories::category::*;

pub(crate) mod syllable {
    pub(crate) const VIRAMA_TERMINATED: u8 = 1;
    pub(crate) const SAKOT_TERMINATED: u8 = 2;
    pub(crate) const STANDARD: u8 = 3;
    pub(crate) const NUMBER_JOINER_TERMINATED: u8 = 4;
    pub(crate) const NUMERAL: u8 = 5;
    pub(crate) const SYMBOL: u8 = 6;
    pub(crate) const HIEROGLYPH: u8 = 7;
    pub(crate) const BROKEN: u8 = 8;
    pub(crate) const NON_CLUSTER: u8 = 9;
}

fn stars(categories: &[u8]) -> Pattern {
    seq(categories.iter().map(|&category| star(one(category))))
}

fn opts(categories: &[u8]) -> Pattern {
    seq(categories.iter().map(|&category| opt(one(category))))
}

fn machine() -> &'static SyllableMachine {
    static MACHINE: OnceLock<SyllableMachine> = OnceLock::new();
    MACHINE.get_or_init(|| {
        let h = any_of(&[H, HVM, IS, SK]);
        let consonant_modifiers = seq([
            stars(&[CMABV, CMBLW]),
            star(seq([
                either([seq([h, one(B)]), one(SUB)]),
                stars(&[CMABV, CMBLW]),
            ])),
        ]);
        let medial_consonants = opts(&[MPRE, MABV, MBLW, MPST]);
        let dependent_vowels = either([stars(&[VPRE, VABV, VBLW, VPST]), one(H)]);
        let vowel_modifiers = seq([opt(one(HVM)), stars(&[VMPRE, VMABV, VMBLW, VMPST])]);
        let final_consonants = stars(&[FABV, FBLW, FPST]);
        let final_modifiers = either([stars(&[FMABV, FMBLW]), opt(one(FMPST))]);
        let complex_syllable_start = seq([opt(any_of(&[R, CS])), any_of(&[B, GB])]);
        let complex_syllable_middle = seq([
            consonant_modifiers.clone(),
            medial_consonants,
            dependent_vowels,
            vowel_modifiers,
            star(seq([one(SK), one(B)])),
        ]);
        let complex_syllable_tail = seq([
            complex_syllable_middle.clone(),
            final_consonants,
            final_modifiers,
        ]);
        let number_joiner_terminated_tail = seq([star(seq([one(HN), one(N)])), one(HN)]);
        let numeral_tail = plus(seq([one(HN), one(N)]));
        let symbol_tail = either([seq([plus(one(SMABV)), star(one(SMBLW))]), plus(one(SMBLW))]);
        let virama_terminated_tail = seq([consonant_modifiers, any_of(&[IS, RK])]);
        let sakot_terminated_tail = seq([complex_syllable_middle, one(SK)]);
        let tail = either([
            complex_syllable_tail.clone(),
            sakot_terminated_tail.clone(),
            symbol_tail,
            virama_terminated_tail.clone(),
        ]);
        let broken_cluster = seq([
            opt(one(R)),
            either([
                tail.clone(),
                number_joiner_terminated_tail.clone(),
                numeral_tail.clone(),
            ]),
        ]);
        let hieroglyph = seq([one(G), opt(one(HR)), opt(one(HM)), star(one(SE))]);
        let hieroglyph_cluster = seq([
            star(one(SB)),
            hieroglyph.clone(),
            star(seq([one(J), star(one(SB)), opt(hieroglyph)])),
        ]);
        let terminated = |pattern: Pattern| seq([pattern, opt(one(ZWNJ))]);
        SyllableMachine::new(&[
            (
                terminated(seq([
                    complex_syllable_start.clone(),
                    virama_terminated_tail,
                ])),
                syllable::VIRAMA_TERMINATED,
            ),
            (
                terminated(seq([complex_syllable_start.clone(), sakot_terminated_tail])),
                syllable::SAKOT_TERMINATED,
            ),
            (
                terminated(seq([complex_syllable_start, complex_syllable_tail])),
                syllable::STANDARD,
            ),
            (
                terminated(seq([one(N), number_joiner_terminated_tail])),
                syllable::NUMBER_JOINER_TERMINATED,
            ),
            (
                terminated(seq([one(N), opt(numeral_tail)])),
                syllable::NUMERAL,
            ),
            (
                terminated(seq([any_of(&[O, GB, SB]), opt(tail)])),
                syllable::SYMBOL,
            ),
            (terminated(hieroglyph_cluster), syllable::HIEROGLYPH),
            (one(FMPST), syllable::NON_CLUSTER),
            (terminated(broken_cluster), syllable::BROKEN),
            (any(), syllable::NON_CLUSTER),
        ])
    })
}

fn is_cgj(info: &GlyphInfo) -> bool {
    info.shaper_category == CGJ
}

fn included(infos: &[GlyphInfo], index: usize) -> bool {
    let info = &infos[index];
    if is_cgj(info) {
        return false;
    }
    if info.shaper_category != ZWNJ {
        return true;
    }
    infos[index + 1..]
        .iter()
        .find(|next| !is_cgj(next))
        .is_none_or(|next| !next.is_unicode_mark())
}

pub(crate) fn find_universal_syllables(buffer: &mut Buffer) {
    find_syllables_skipping(buffer, machine(), syllable::BROKEN, included);
}

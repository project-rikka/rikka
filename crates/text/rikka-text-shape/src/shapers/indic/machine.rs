use std::sync::OnceLock;

use crate::buffer::glyph_buffer::Buffer;
use crate::shapers::indic::properties::category::*;
use crate::shapers::syllabic::machine::SyllableMachine;
use crate::shapers::syllabic::pattern::{any, any_of, either, one, opt, seq, star};
use crate::shapers::syllabic::syllables::find_syllables;

pub(crate) mod syllable {
    pub(crate) const CONSONANT: u8 = 0;
    pub(crate) const VOWEL: u8 = 1;
    pub(crate) const STANDALONE: u8 = 2;
    pub(crate) const SYMBOL: u8 = 3;
    pub(crate) const BROKEN: u8 = 4;
    pub(crate) const NON_INDIC: u8 = 5;
}

fn machine() -> &'static SyllableMachine {
    static MACHINE: OnceLock<SyllableMachine> = OnceLock::new();
    MACHINE.get_or_init(|| {
        let c = any_of(&[C, RA]);
        let n = seq([
            opt(seq([opt(one(ZWNJ)), one(RS)])),
            opt(seq([one(N), opt(one(N))])),
        ]);
        let z = any_of(&[ZWJ, ZWNJ]);
        let reph = either([seq([one(RA), one(H)]), one(REPHA)]);
        let sm = any_of(&[SM, SMPST]);
        let cn = seq([c, opt(one(ZWJ)), opt(n.clone())]);
        let symbol = seq([one(SYMBOL), opt(one(N))]);
        let matra_group = seq([
            star(z.clone()),
            either([one(M), seq([opt(sm.clone()), one(MPST)])]),
            opt(one(N)),
            opt(one(H)),
        ]);
        let syllable_tail = seq([
            opt(seq([opt(z.clone()), sm.clone(), opt(sm), opt(one(ZWNJ))])),
            star(one(A)),
        ]);
        let halant_group = seq([opt(z), one(H), opt(seq([one(ZWJ), opt(one(N))]))]);
        let final_halant_group = either([halant_group.clone(), seq([one(H), one(ZWNJ)])]);
        let halant_or_matra_group = either([final_halant_group, star(matra_group)]);
        let complex_syllable_tail = seq([
            star(seq([halant_group, cn.clone()])),
            opt(one(CM)),
            halant_or_matra_group,
            syllable_tail.clone(),
        ]);
        let consonant_syllable =
            seq([opt(any_of(&[REPHA, CS])), cn, complex_syllable_tail.clone()]);
        let vowel_syllable = seq([
            opt(reph.clone()),
            one(V),
            opt(n.clone()),
            either([one(ZWJ), complex_syllable_tail.clone()]),
        ]);
        let standalone_cluster = seq([
            either([
                seq([opt(any_of(&[REPHA, CS])), one(PLACEHOLDER)]),
                seq([opt(reph.clone()), one(DOTTED_CIRCLE)]),
            ]),
            opt(n.clone()),
            complex_syllable_tail.clone(),
        ]);
        let symbol_cluster = seq([symbol, syllable_tail]);
        let broken_cluster = seq([opt(reph), opt(n), complex_syllable_tail]);
        SyllableMachine::new(&[
            (consonant_syllable, syllable::CONSONANT),
            (vowel_syllable, syllable::VOWEL),
            (standalone_cluster, syllable::STANDALONE),
            (symbol_cluster, syllable::SYMBOL),
            (one(SMPST), syllable::NON_INDIC),
            (broken_cluster, syllable::BROKEN),
            (any(), syllable::NON_INDIC),
        ])
    })
}

pub(crate) fn find_indic_syllables(buffer: &mut Buffer) {
    find_syllables(buffer, machine(), syllable::BROKEN);
}

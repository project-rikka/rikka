use std::sync::OnceLock;

use crate::buffer::glyph_buffer::Buffer;
use crate::shapers::indic::properties::category::*;
use crate::shapers::syllabic::machine::SyllableMachine;
use crate::shapers::syllabic::pattern::{any, any_of, either, one, opt, seq, star};
use crate::shapers::syllabic::syllables::find_syllables;

pub(crate) mod syllable {
    pub(crate) const CONSONANT: u8 = 0;
    pub(crate) const BROKEN: u8 = 2;
    pub(crate) const NON_MYANMAR: u8 = 3;
}

const INDEPENDENT_VOWEL: u8 = V;
const DOT_BELOW: u8 = N;
const GENERIC_BASE: u8 = PLACEHOLDER;

fn machine() -> &'static SyllableMachine {
    static MACHINE: OnceLock<SyllableMachine> = OnceLock::new();
    MACHINE.get_or_init(|| {
        let joiner = any_of(&[ZWJ, ZWNJ]);
        let kinzi = seq([one(RA), one(AS), one(H)]);
        let sm = any_of(&[SM, SMPST]);
        let c = any_of(&[C, RA]);
        let dot_below = seq([one(DOT_BELOW), opt(one(AS))]);
        let medial_group = seq([
            opt(one(MY)),
            opt(one(AS)),
            opt(one(MR)),
            opt(seq([
                either([
                    seq([one(MW), opt(one(MH)), opt(one(ML))]),
                    seq([one(MH), opt(one(ML))]),
                    one(ML),
                ]),
                opt(one(AS)),
            ])),
        ]);
        let main_vowel_group = seq([
            star(seq([one(VPRE), opt(one(VS))])),
            star(one(VABV)),
            star(one(VBLW)),
            star(one(A)),
            opt(dot_below.clone()),
        ]);
        let post_vowel_group = seq([
            one(VPST),
            opt(one(MH)),
            opt(one(ML)),
            star(one(AS)),
            star(one(VABV)),
            star(one(A)),
            opt(dot_below),
        ]);
        let tone_group = either([
            sm,
            seq([one(PT), star(one(A)), opt(one(DOT_BELOW)), opt(one(AS))]),
        ]);
        let complex_syllable_tail = seq([
            star(one(AS)),
            medial_group,
            main_vowel_group,
            star(post_vowel_group),
            star(tone_group),
            opt(joiner.clone()),
        ]);
        let syllable_tail = seq([
            star(seq([
                one(H),
                either([c.clone(), one(INDEPENDENT_VOWEL)]),
                opt(one(VS)),
            ])),
            either([one(H), complex_syllable_tail]),
        ]);
        let consonant_syllable = seq([
            opt(either([kinzi.clone(), one(CS)])),
            either([
                c,
                one(INDEPENDENT_VOWEL),
                one(GENERIC_BASE),
                one(DOTTED_CIRCLE),
            ]),
            opt(one(VS)),
            syllable_tail.clone(),
        ]);
        let broken_cluster = seq([opt(kinzi), opt(one(VS)), syllable_tail]);
        SyllableMachine::new(&[
            (consonant_syllable, syllable::CONSONANT),
            (either([joiner, one(SMPST)]), syllable::NON_MYANMAR),
            (broken_cluster, syllable::BROKEN),
            (any(), syllable::NON_MYANMAR),
        ])
    })
}

pub(crate) fn find_myanmar_syllables(buffer: &mut Buffer) {
    find_syllables(buffer, machine(), syllable::BROKEN);
}

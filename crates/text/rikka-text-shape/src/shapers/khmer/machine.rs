use std::sync::OnceLock;

use crate::buffer::glyph_buffer::Buffer;
use crate::shapers::indic::properties::category::*;
use crate::shapers::syllabic::machine::SyllableMachine;
use crate::shapers::syllabic::pattern::{any, any_of, either, one, opt, seq, star};
use crate::shapers::syllabic::syllables::find_syllables;

pub(crate) mod syllable {
    pub(crate) const CONSONANT: u8 = 0;
    pub(crate) const BROKEN: u8 = 1;
    pub(crate) const NON_KHMER: u8 = 2;
}

fn machine() -> &'static SyllableMachine {
    static MACHINE: OnceLock<SyllableMachine> = OnceLock::new();
    MACHINE.get_or_init(|| {
        let c = any_of(&[C, RA, V]);
        let joiner = any_of(&[ZWJ, ZWNJ]);
        let cn = seq([c.clone(), opt(seq([opt(joiner.clone()), one(ROBATIC)]))]);
        let xgroup = star(seq([star(joiner.clone()), one(XGROUP)]));
        let ygroup = star(one(YGROUP));
        let matra_group = seq([
            opt(one(VPRE)),
            xgroup.clone(),
            opt(one(VBLW)),
            xgroup.clone(),
            opt(seq([opt(joiner), one(VABV)])),
            xgroup.clone(),
            opt(one(VPST)),
        ]);
        let syllable_tail = seq([
            xgroup.clone(),
            matra_group,
            xgroup,
            opt(seq([one(H), c])),
            ygroup,
        ]);
        let broken_cluster = seq([
            opt(one(ROBATIC)),
            star(seq([one(H), cn.clone()])),
            either([one(H), syllable_tail]),
        ]);
        let consonant_syllable = seq([
            either([cn, one(PLACEHOLDER), one(DOTTED_CIRCLE)]),
            broken_cluster.clone(),
        ]);
        SyllableMachine::new(&[
            (consonant_syllable, syllable::CONSONANT),
            (broken_cluster, syllable::BROKEN),
            (any(), syllable::NON_KHMER),
        ])
    })
}

pub(crate) fn find_khmer_syllables(buffer: &mut Buffer) {
    find_syllables(buffer, machine(), syllable::BROKEN);
}

const SYLLABLE_BASE: u32 = 0xAC00;
const LEADING_BASE: u32 = 0x1100;
const VOWEL_BASE: u32 = 0x1161;
const TRAILING_BASE: u32 = 0x11A7;
const LEADING_COUNT: u32 = 19;
const VOWEL_COUNT: u32 = 21;
const TRAILING_COUNT: u32 = 28;
const BLOCK_COUNT: u32 = VOWEL_COUNT * TRAILING_COUNT;
const SYLLABLE_COUNT: u32 = LEADING_COUNT * BLOCK_COUNT;

#[inline]
pub(crate) fn decompose(codepoint: u32) -> Option<(u32, u32)> {
    let syllable = codepoint.wrapping_sub(SYLLABLE_BASE);
    if syllable >= SYLLABLE_COUNT {
        return None;
    }
    let trailing = syllable % TRAILING_COUNT;
    Some(if trailing == 0 {
        (
            LEADING_BASE + syllable / BLOCK_COUNT,
            VOWEL_BASE + (syllable % BLOCK_COUNT) / TRAILING_COUNT,
        )
    } else {
        (codepoint - trailing, TRAILING_BASE + trailing)
    })
}

#[inline]
pub(crate) fn compose(first: u32, second: u32) -> Option<Option<u32>> {
    let leading = first.wrapping_sub(LEADING_BASE);
    if leading < LEADING_COUNT {
        let vowel = second.wrapping_sub(VOWEL_BASE);
        return Some(
            (vowel < VOWEL_COUNT)
                .then(|| SYLLABLE_BASE + (leading * VOWEL_COUNT + vowel) * TRAILING_COUNT),
        );
    }
    let syllable = first.wrapping_sub(SYLLABLE_BASE);
    if syllable < SYLLABLE_COUNT && syllable.is_multiple_of(TRAILING_COUNT) {
        let trailing = second.wrapping_sub(TRAILING_BASE);
        return Some((trailing.wrapping_sub(1) < TRAILING_COUNT - 1).then(|| first + trailing));
    }
    None
}

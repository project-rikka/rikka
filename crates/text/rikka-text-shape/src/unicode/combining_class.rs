pub(crate) const ATTACHED_BELOW_LEFT: u8 = 200;
pub(crate) const ATTACHED_BELOW: u8 = 202;
pub(crate) const ATTACHED_ABOVE: u8 = 214;
pub(crate) const ATTACHED_ABOVE_RIGHT: u8 = 216;
pub(crate) const BELOW_LEFT: u8 = 218;
pub(crate) const BELOW: u8 = 220;
pub(crate) const BELOW_RIGHT: u8 = 222;
pub(crate) const ABOVE_LEFT: u8 = 228;
pub(crate) const ABOVE: u8 = 230;
pub(crate) const ABOVE_RIGHT: u8 = 232;
pub(crate) const DOUBLE_BELOW: u8 = 233;
pub(crate) const DOUBLE_ABOVE: u8 = 234;

pub(crate) const HEBREW_SHEVA: u8 = 22;
pub(crate) const HEBREW_HATAF_SEGOL: u8 = 15;
pub(crate) const HEBREW_HATAF_PATAH: u8 = 16;
pub(crate) const HEBREW_HATAF_QAMATS: u8 = 17;
pub(crate) const HEBREW_HIRIQ: u8 = 23;
pub(crate) const HEBREW_TSERE: u8 = 18;
pub(crate) const HEBREW_SEGOL: u8 = 19;
pub(crate) const HEBREW_PATAH: u8 = 20;
pub(crate) const HEBREW_QAMATS: u8 = 21;
pub(crate) const HEBREW_HOLAM: u8 = 14;
pub(crate) const HEBREW_QUBUTS: u8 = 24;
pub(crate) const HEBREW_DAGESH: u8 = 12;
pub(crate) const HEBREW_METEG: u8 = 25;
pub(crate) const HEBREW_RAFE: u8 = 13;
pub(crate) const HEBREW_SHIN_DOT: u8 = 10;
pub(crate) const HEBREW_SIN_DOT: u8 = 11;
pub(crate) const HEBREW_POINT_VARIKA: u8 = 26;

pub(crate) const ARABIC_FATHATAN: u8 = 28;
pub(crate) const ARABIC_DAMMATAN: u8 = 29;
pub(crate) const ARABIC_KASRATAN: u8 = 30;
pub(crate) const ARABIC_FATHA: u8 = 31;
pub(crate) const ARABIC_DAMMA: u8 = 32;
pub(crate) const ARABIC_KASRA: u8 = 33;
pub(crate) const ARABIC_SHADDA: u8 = 27;
pub(crate) const ARABIC_SUKUN: u8 = 34;
pub(crate) const ARABIC_SUPERSCRIPT_ALEF: u8 = 35;
pub(crate) const SYRIAC_SUPERSCRIPT_ALAPH: u8 = 36;

pub(crate) const THAI_SARA_U: u8 = 3;
pub(crate) const THAI_MAI: u8 = 107;
pub(crate) const LAO_SIGN_U: u8 = 118;
pub(crate) const LAO_MAI: u8 = 122;
pub(crate) const TIBETAN_SIGN_AA: u8 = 129;
pub(crate) const TIBETAN_SIGN_I: u8 = 132;
pub(crate) const TIBETAN_SIGN_U: u8 = 131;

static MODIFIED: [u8; 256] = {
    let mut table = [0u8; 256];
    let mut class = 0;
    while class < 256 {
        table[class] = class as u8;
        class += 1;
    }
    table[10] = HEBREW_SHEVA;
    table[11] = HEBREW_HATAF_SEGOL;
    table[12] = HEBREW_HATAF_PATAH;
    table[13] = HEBREW_HATAF_QAMATS;
    table[14] = HEBREW_HIRIQ;
    table[15] = HEBREW_TSERE;
    table[16] = HEBREW_SEGOL;
    table[17] = HEBREW_PATAH;
    table[18] = HEBREW_QAMATS;
    table[19] = HEBREW_HOLAM;
    table[20] = HEBREW_QUBUTS;
    table[21] = HEBREW_DAGESH;
    table[22] = HEBREW_METEG;
    table[23] = HEBREW_RAFE;
    table[24] = HEBREW_SHIN_DOT;
    table[25] = HEBREW_SIN_DOT;
    table[26] = HEBREW_POINT_VARIKA;
    table[27] = ARABIC_FATHATAN;
    table[28] = ARABIC_DAMMATAN;
    table[29] = ARABIC_KASRATAN;
    table[30] = ARABIC_FATHA;
    table[31] = ARABIC_DAMMA;
    table[32] = ARABIC_KASRA;
    table[33] = ARABIC_SHADDA;
    table[34] = ARABIC_SUKUN;
    table[35] = ARABIC_SUPERSCRIPT_ALEF;
    table[36] = SYRIAC_SUPERSCRIPT_ALAPH;
    table[84] = 0;
    table[91] = 0;
    table[103] = THAI_SARA_U;
    table[130] = TIBETAN_SIGN_I;
    table[132] = TIBETAN_SIGN_U;
    table
};

#[inline]
pub(crate) fn modified_combining_class(codepoint: u32, combining_class: u8) -> u8 {
    match codepoint {
        0x1A60 | 0x0FC6 => 254,
        0x0F39 => 127,
        _ => MODIFIED[usize::from(combining_class)],
    }
}

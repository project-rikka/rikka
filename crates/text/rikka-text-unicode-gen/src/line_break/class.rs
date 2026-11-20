use crate::error::{Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Class {
    Aksara,
    Alphabetic,
    AksaraPrebase,
    AksaraStart,
    BreakBoth,
    BreakAfter,
    BreakBefore,
    MandatoryBreak,
    ContingentBreak,
    ClosePunctuation,
    CombiningMark,
    CloseParenthesis,
    CarriageReturn,
    EBase,
    EModifier,
    Exclamation,
    Glue,
    H2,
    H3,
    UnambiguousHyphen,
    HebrewLetter,
    Hyphen,
    Ideographic,
    Inseparable,
    InfixNumeric,
    Jl,
    Jt,
    Jv,
    LineFeed,
    NextLine,
    Nonstarter,
    Numeric,
    OpenPunctuation,
    PostfixNumeric,
    PrefixNumeric,
    Quotation,
    RegionalIndicator,
    Space,
    BreakSymbols,
    ViramaFinal,
    Virama,
    WordJoiner,
    ZWSpace,
    Zwj,
}

impl Class {
    pub(crate) fn resolve(line_break: &str, general_category: &str) -> Result<Self> {
        use Class as L;
        Ok(match line_break {
            "AI" | "SG" | "XX" => L::Alphabetic,
            "CJ" | "NS" => L::Nonstarter,
            "SA" if matches!(general_category, "Mn" | "Mc") => L::CombiningMark,
            "SA" => L::Alphabetic,
            "AK" => L::Aksara,
            "AL" => L::Alphabetic,
            "AP" => L::AksaraPrebase,
            "AS" => L::AksaraStart,
            "B2" => L::BreakBoth,
            "BA" => L::BreakAfter,
            "BB" => L::BreakBefore,
            "BK" => L::MandatoryBreak,
            "CB" => L::ContingentBreak,
            "CL" => L::ClosePunctuation,
            "CM" => L::CombiningMark,
            "CP" => L::CloseParenthesis,
            "CR" => L::CarriageReturn,
            "EB" => L::EBase,
            "EM" => L::EModifier,
            "EX" => L::Exclamation,
            "GL" => L::Glue,
            "H2" => L::H2,
            "H3" => L::H3,
            "HH" => L::UnambiguousHyphen,
            "HL" => L::HebrewLetter,
            "HY" => L::Hyphen,
            "ID" => L::Ideographic,
            "IN" => L::Inseparable,
            "IS" => L::InfixNumeric,
            "JL" => L::Jl,
            "JT" => L::Jt,
            "JV" => L::Jv,
            "LF" => L::LineFeed,
            "NL" => L::NextLine,
            "NU" => L::Numeric,
            "OP" => L::OpenPunctuation,
            "PO" => L::PostfixNumeric,
            "PR" => L::PrefixNumeric,
            "QU" => L::Quotation,
            "RI" => L::RegionalIndicator,
            "SP" => L::Space,
            "SY" => L::BreakSymbols,
            "VF" => L::ViramaFinal,
            "VI" => L::Virama,
            "WJ" => L::WordJoiner,
            "ZW" => L::ZWSpace,
            "ZWJ" => L::Zwj,
            other => {
                return Err(Error::Data(format!(
                    "line break class `{other}` has no resolution"
                )));
            }
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Classes(u64);

impl Classes {
    pub(crate) const fn new(classes: &[Class]) -> Self {
        let mut bits = 0;
        let mut index = 0;
        while index < classes.len() {
            bits |= 1 << classes[index] as u64;
            index += 1;
        }
        Self(bits)
    }

    pub(crate) const fn has(self, class: Class) -> bool {
        self.0 & (1 << class as u64) != 0
    }
}

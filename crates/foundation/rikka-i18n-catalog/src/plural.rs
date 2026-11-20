use std::ops::RangeInclusive;

use crate::args::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluralCategory {
    Zero,
    One,
    Two,
    Few,
    Many,
    Other,
}

impl PluralCategory {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "zero" => Self::Zero,
            "one" => Self::One,
            "two" => Self::Two,
            "few" => Self::Few,
            "many" => Self::Many,
            "other" => Self::Other,
            _ => return None,
        })
    }

    pub fn select(language: &str, kind: PluralKind, ops: PluralOperands) -> Self {
        match kind {
            PluralKind::Cardinal => cardinal(rule_set(language), ops),
            PluralKind::Ordinal => ordinal(language, ops),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluralKind {
    Cardinal,
    Ordinal,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PluralOperands {
    n: f64,
    i: u64,
    v: u32,
    f: u64,
    t: u64,
}

impl PluralOperands {
    pub fn integer(value: i64) -> Self {
        let i = value.unsigned_abs();
        Self {
            n: i as f64,
            i,
            v: 0,
            f: 0,
            t: 0,
        }
    }

    pub fn decimal(text: &str) -> Option<Self> {
        let text = text.trim();
        let digits = text.strip_prefix(['-', '+']).unwrap_or(text);
        let (int, fraction) = digits.split_once('.').unwrap_or((digits, ""));
        if int.is_empty() || !int.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        if !fraction.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }

        Some(Self {
            n: digits.parse().ok()?,
            i: int.parse().unwrap_or(u64::MAX),
            v: fraction.len() as u32,
            f: fraction.parse().unwrap_or(0),
            t: fraction.trim_end_matches('0').parse().unwrap_or(0),
        })
    }

    pub fn from_value(value: Value<'_>) -> Option<Self> {
        match value {
            Value::Int(value) => Some(Self::integer(value)),
            Value::Float(value) => Self::decimal(&value.to_string()),
            Value::Text(text) => Self::decimal(text),
        }
    }

    fn n_is(self, value: u64) -> bool {
        self.n == value as f64
    }

    fn n_in(self, range: RangeInclusive<u64>) -> bool {
        self.v == 0 && range.contains(&self.i)
    }

    fn n_mod_is(self, modulus: u64, value: u64) -> bool {
        self.n % modulus as f64 == value as f64
    }

    fn n_mod_in(self, modulus: u64, range: RangeInclusive<u64>) -> bool {
        self.v == 0 && range.contains(&(self.i % modulus))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RuleSet {
    Invariant,
    IntegerOne,
    ExactlyOne,
    UpToOne,
    BelowTwo,
    OneAndTwo,
    Danish,
    Icelandic,
    Filipino,
    Macedonian,
    Slavic,
    Polish,
    Czech,
    Lithuanian,
    Latvian,
    Arabic,
    Hebrew,
    Slovenian,
    Sorbian,
    Serbian,
    Romanian,
    Welsh,
    Irish,
    Maltese,
    Gaelic,
    Breton,
}

fn rule_set(language: &str) -> RuleSet {
    match language {
        "bm" | "bo" | "dz" | "hnj" | "id" | "ig" | "ii" | "in" | "ja" | "jbo" | "jv" | "jw"
        | "kde" | "kea" | "km" | "ko" | "lkt" | "lo" | "ms" | "my" | "nqo" | "osa" | "sah"
        | "ses" | "sg" | "su" | "th" | "to" | "tpi" | "vi" | "wo" | "yo" | "yue" | "zh" => {
            RuleSet::Invariant
        }

        "ast" | "ca" | "de" | "en" | "et" | "fi" | "fy" | "gl" | "ia" | "io" | "it" | "ji"
        | "nl" | "sc" | "scn" | "sv" | "sw" | "ur" | "yi" => RuleSet::IntegerOne,

        "af" | "an" | "asa" | "az" | "bem" | "bez" | "bg" | "brx" | "ce" | "cgg" | "chr"
        | "ckb" | "dv" | "ee" | "el" | "eo" | "es" | "eu" | "fo" | "fur" | "gsw" | "ha" | "haw"
        | "hu" | "jgo" | "jmc" | "ka" | "kaj" | "kcg" | "kk" | "kkj" | "kl" | "ks" | "ksb"
        | "ku" | "ky" | "lb" | "lg" | "mas" | "mgo" | "ml" | "mn" | "mr" | "nah" | "nb" | "nd"
        | "ne" | "nn" | "nnh" | "no" | "nr" | "ny" | "nyn" | "om" | "or" | "os" | "pap" | "ps"
        | "rm" | "rof" | "rwk" | "saq" | "sd" | "sdh" | "seh" | "sn" | "so" | "sq" | "ss"
        | "ssy" | "st" | "syr" | "ta" | "te" | "teo" | "tig" | "tk" | "tn" | "tr" | "ts" | "ug"
        | "uz" | "ve" | "vo" | "vun" | "wae" | "xh" | "xog" => RuleSet::ExactlyOne,

        "ff" | "fr" | "hy" | "kab" | "pt" => RuleSet::UpToOne,
        "am" | "as" | "bn" | "doi" | "fa" | "gu" | "hi" | "kn" | "pcm" | "zu" => RuleSet::BelowTwo,
        "iu" | "naq" | "sat" | "se" | "sma" | "smi" | "smj" | "smn" | "sms" => RuleSet::OneAndTwo,

        "da" => RuleSet::Danish,
        "is" => RuleSet::Icelandic,
        "fil" | "tl" => RuleSet::Filipino,
        "mk" => RuleSet::Macedonian,
        "be" | "ru" | "uk" => RuleSet::Slavic,
        "pl" => RuleSet::Polish,
        "cs" | "sk" => RuleSet::Czech,
        "lt" => RuleSet::Lithuanian,
        "lv" | "prg" => RuleSet::Latvian,
        "ar" | "ars" => RuleSet::Arabic,
        "he" | "iw" => RuleSet::Hebrew,
        "sl" => RuleSet::Slovenian,
        "dsb" | "hsb" => RuleSet::Sorbian,
        "bs" | "hr" | "sh" | "sr" => RuleSet::Serbian,
        "mo" | "ro" => RuleSet::Romanian,
        "cy" => RuleSet::Welsh,
        "ga" => RuleSet::Irish,
        "mt" => RuleSet::Maltese,
        "gd" => RuleSet::Gaelic,
        "br" => RuleSet::Breton,

        _ => RuleSet::ExactlyOne,
    }
}

fn cardinal(rules: RuleSet, ops: PluralOperands) -> PluralCategory {
    use PluralCategory::{Few, Many, One, Other, Two, Zero};

    let PluralOperands { i, v, f, t, .. } = ops;
    let i_mod = |modulus: u64| i % modulus;
    let f_mod = |modulus: u64| f % modulus;

    match rules {
        RuleSet::Invariant => Other,

        RuleSet::IntegerOne if i == 1 && v == 0 => One,
        RuleSet::ExactlyOne if ops.n_is(1) => One,
        RuleSet::UpToOne if v == 0 && (i == 0 || i == 1) => One,
        RuleSet::BelowTwo if i == 0 || ops.n_is(1) => One,
        RuleSet::IntegerOne | RuleSet::ExactlyOne | RuleSet::UpToOne | RuleSet::BelowTwo => Other,

        RuleSet::OneAndTwo => match () {
            _ if ops.n_is(1) => One,
            _ if ops.n_is(2) => Two,
            _ => Other,
        },

        RuleSet::Danish if ops.n_is(1) || (t != 0 && (i == 0 || i == 1)) => One,
        RuleSet::Danish => Other,

        RuleSet::Icelandic if t != 0 || (i_mod(10) == 1 && i_mod(100) != 11) => One,
        RuleSet::Icelandic => Other,

        RuleSet::Filipino
            if (v == 0 && !matches!(i_mod(10), 4 | 6 | 9))
                || (v != 0 && !matches!(f_mod(10), 4 | 6 | 9)) =>
        {
            One
        }
        RuleSet::Filipino => Other,

        RuleSet::Macedonian
            if (v == 0 && i_mod(10) == 1 && i_mod(100) != 11)
                || (f_mod(10) == 1 && f_mod(100) != 11) =>
        {
            One
        }
        RuleSet::Macedonian => Other,

        RuleSet::Slavic => match () {
            _ if v != 0 => Other,
            _ if i_mod(10) == 1 && i_mod(100) != 11 => One,
            _ if (2..=4).contains(&i_mod(10)) && !(12..=14).contains(&i_mod(100)) => Few,
            _ => Many,
        },

        RuleSet::Polish => match () {
            _ if v != 0 => Other,
            _ if i == 1 => One,
            _ if (2..=4).contains(&i_mod(10)) && !(12..=14).contains(&i_mod(100)) => Few,
            _ => Many,
        },

        RuleSet::Czech => match () {
            _ if v != 0 => Many,
            _ if i == 1 => One,
            _ if (2..=4).contains(&i) => Few,
            _ => Other,
        },

        RuleSet::Lithuanian => match () {
            _ if ops.n_mod_is(10, 1) && !ops.n_mod_in(100, 11..=19) => One,
            _ if ops.n_mod_in(10, 2..=9) && !ops.n_mod_in(100, 11..=19) => Few,
            _ if f != 0 => Many,
            _ => Other,
        },

        RuleSet::Latvian => match () {
            _ if ops.n_mod_is(10, 0)
                || ops.n_mod_in(100, 11..=19)
                || (v == 2 && (11..=19).contains(&f_mod(100))) =>
            {
                Zero
            }
            _ if (ops.n_mod_is(10, 1) && !ops.n_mod_is(100, 11))
                || (v == 2 && f_mod(10) == 1 && f_mod(100) != 11)
                || (v != 2 && f_mod(10) == 1) =>
            {
                One
            }
            _ => Other,
        },

        RuleSet::Arabic => match () {
            _ if ops.n_is(0) => Zero,
            _ if ops.n_is(1) => One,
            _ if ops.n_is(2) => Two,
            _ if ops.n_mod_in(100, 3..=10) => Few,
            _ if ops.n_mod_in(100, 11..=99) => Many,
            _ => Other,
        },

        RuleSet::Hebrew => match () {
            _ if v != 0 => Other,
            _ if i == 1 => One,
            _ if i == 2 => Two,
            _ => Other,
        },

        RuleSet::Slovenian => match () {
            _ if v == 0 && i_mod(100) == 1 => One,
            _ if v == 0 && i_mod(100) == 2 => Two,
            _ if v != 0 || (3..=4).contains(&i_mod(100)) => Few,
            _ => Other,
        },

        RuleSet::Sorbian => match () {
            _ if (v == 0 && i_mod(100) == 1) || f_mod(100) == 1 => One,
            _ if (v == 0 && i_mod(100) == 2) || f_mod(100) == 2 => Two,
            _ if (v == 0 && (3..=4).contains(&i_mod(100))) || (3..=4).contains(&f_mod(100)) => Few,
            _ => Other,
        },

        RuleSet::Serbian => match () {
            _ if (v == 0 && i_mod(10) == 1 && i_mod(100) != 11)
                || (f_mod(10) == 1 && f_mod(100) != 11) =>
            {
                One
            }
            _ if (v == 0 && (2..=4).contains(&i_mod(10)) && !(12..=14).contains(&i_mod(100)))
                || ((2..=4).contains(&f_mod(10)) && !(12..=14).contains(&f_mod(100))) =>
            {
                Few
            }
            _ => Other,
        },

        RuleSet::Romanian => match () {
            _ if v == 0 && i == 1 => One,
            _ if v != 0 || ops.n_is(0) || (!ops.n_is(1) && ops.n_mod_in(100, 1..=19)) => Few,
            _ => Other,
        },

        RuleSet::Welsh => match () {
            _ if ops.n_is(0) => Zero,
            _ if ops.n_is(1) => One,
            _ if ops.n_is(2) => Two,
            _ if ops.n_is(3) => Few,
            _ if ops.n_is(6) => Many,
            _ => Other,
        },

        RuleSet::Irish => match () {
            _ if ops.n_is(1) => One,
            _ if ops.n_is(2) => Two,
            _ if ops.n_in(3..=6) => Few,
            _ if ops.n_in(7..=10) => Many,
            _ => Other,
        },

        RuleSet::Maltese => match () {
            _ if ops.n_is(1) => One,
            _ if ops.n_is(2) => Two,
            _ if ops.n_is(0) || ops.n_mod_in(100, 3..=10) => Few,
            _ if ops.n_mod_in(100, 11..=19) => Many,
            _ => Other,
        },

        RuleSet::Gaelic => match () {
            _ if ops.n_is(1) || ops.n_is(11) => One,
            _ if ops.n_is(2) || ops.n_is(12) => Two,
            _ if ops.n_in(3..=10) || ops.n_in(13..=19) => Few,
            _ => Other,
        },

        RuleSet::Breton => match () {
            _ if ops.n_mod_is(10, 1)
                && !ops.n_mod_is(100, 11)
                && !ops.n_mod_is(100, 71)
                && !ops.n_mod_is(100, 91) =>
            {
                One
            }
            _ if ops.n_mod_is(10, 2)
                && !ops.n_mod_is(100, 12)
                && !ops.n_mod_is(100, 72)
                && !ops.n_mod_is(100, 92) =>
            {
                Two
            }
            _ if (ops.n_mod_in(10, 3..=4) || ops.n_mod_is(10, 9))
                && !ops.n_mod_in(100, 10..=19)
                && !ops.n_mod_in(100, 70..=79)
                && !ops.n_mod_in(100, 90..=99) =>
            {
                Few
            }
            _ if !ops.n_is(0) && ops.n_mod_is(1_000_000, 0) => Many,
            _ => Other,
        },
    }
}

fn ordinal(language: &str, ops: PluralOperands) -> PluralCategory {
    use PluralCategory::{Few, Many, One, Other, Two, Zero};

    let i = ops.i;

    match language {
        "en" => match () {
            _ if ops.n_mod_is(10, 1) && !ops.n_mod_is(100, 11) => One,
            _ if ops.n_mod_is(10, 2) && !ops.n_mod_is(100, 12) => Two,
            _ if ops.n_mod_is(10, 3) && !ops.n_mod_is(100, 13) => Few,
            _ => Other,
        },
        "it" | "sc" | "scn" | "vec" => match () {
            _ if ops.n_is(8) || ops.n_is(11) || ops.n_is(80) || ops.n_is(800) => Many,
            _ => Other,
        },
        "sv" => match () {
            _ if (ops.n_mod_is(10, 1) || ops.n_mod_is(10, 2))
                && !ops.n_mod_is(100, 11)
                && !ops.n_mod_is(100, 12) =>
            {
                One
            }
            _ => Other,
        },
        "hu" => match () {
            _ if ops.n_is(1) || ops.n_is(5) => One,
            _ => Other,
        },
        "ka" => match () {
            _ if i == 1 => One,
            _ if i == 0 || matches!(i % 100, 2..=20 | 40 | 60 | 80) => Many,
            _ => Other,
        },
        "uk" => match () {
            _ if ops.n_mod_is(10, 3) && !ops.n_mod_is(100, 13) => Few,
            _ => Other,
        },
        "be" => match () {
            _ if (ops.n_mod_is(10, 2) || ops.n_mod_is(10, 3))
                && !ops.n_mod_is(100, 12)
                && !ops.n_mod_is(100, 13) =>
            {
                Few
            }
            _ => Other,
        },
        "sq" => match () {
            _ if ops.n_is(1) => One,
            _ if ops.n_mod_is(10, 4) && !ops.n_mod_is(100, 14) => Many,
            _ => Other,
        },
        "cy" => match () {
            _ if ops.n_is(0) || ops.n_in(7..=9) => Zero,
            _ if ops.n_is(1) => One,
            _ if ops.n_is(2) => Two,
            _ if ops.n_in(3..=4) => Few,
            _ if ops.n_in(5..=6) => Many,
            _ => Other,
        },
        "hy" => match () {
            _ if i == 1 => One,
            _ => Other,
        },
        "ne" => match () {
            _ if ops.n_in(1..=4) => One,
            _ => Other,
        },
        "mr" => match () {
            _ if ops.n_is(1) => One,
            _ if ops.n_in(2..=3) => Two,
            _ if ops.n_is(4) => Few,
            _ => Other,
        },
        "as" | "bn" => match () {
            _ if ops.n_is(1) || ops.n_is(5) || ops.n_in(7..=10) => One,
            _ if ops.n_in(2..=3) => Two,
            _ if ops.n_is(4) => Few,
            _ if ops.n_is(6) => Many,
            _ => Other,
        },
        "gu" => match () {
            _ if ops.n_is(1) => One,
            _ if ops.n_in(2..=3) => Two,
            _ if ops.n_is(4) => Few,
            _ if ops.n_is(6) => Many,
            _ => Other,
        },
        _ => Other,
    }
}

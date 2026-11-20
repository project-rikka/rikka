use std::cell::Cell;

use super::class::{Class, Classes};
use super::unit::Unit;

use Class as L;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Decision {
    Allowed,
    Mandatory,
}

const HARD_BREAKS: Classes = Classes::new(&[
    L::MandatoryBreak,
    L::CarriageReturn,
    L::LineFeed,
    L::NextLine,
]);
const SPACES: Classes = Classes::new(&[L::Space, L::ZWSpace]);
const GLUE_BREAKERS: Classes =
    Classes::new(&[L::Space, L::BreakAfter, L::Hyphen, L::UnambiguousHyphen]);
const CLOSERS: Classes = Classes::new(&[
    L::ClosePunctuation,
    L::CloseParenthesis,
    L::Exclamation,
    L::BreakSymbols,
]);
const QUOTE_OPENERS: Classes = Classes::new(&[
    L::MandatoryBreak,
    L::CarriageReturn,
    L::LineFeed,
    L::NextLine,
    L::OpenPunctuation,
    L::Quotation,
    L::Glue,
    L::Space,
    L::ZWSpace,
]);
const QUOTE_CLOSERS: Classes = Classes::new(&[
    L::Space,
    L::Glue,
    L::WordJoiner,
    L::ClosePunctuation,
    L::Quotation,
    L::CloseParenthesis,
    L::Exclamation,
    L::InfixNumeric,
    L::BreakSymbols,
    L::MandatoryBreak,
    L::CarriageReturn,
    L::LineFeed,
    L::NextLine,
    L::ZWSpace,
]);
const CLOSING: Classes = Classes::new(&[L::ClosePunctuation, L::CloseParenthesis]);
const HYPHENS: Classes = Classes::new(&[L::Hyphen, L::UnambiguousHyphen]);
const LETTERS: Classes = Classes::new(&[L::Alphabetic, L::HebrewLetter]);
const WORD_STARTS: Classes = Classes::new(&[
    L::MandatoryBreak,
    L::CarriageReturn,
    L::LineFeed,
    L::NextLine,
    L::Space,
    L::ZWSpace,
    L::ContingentBreak,
    L::Glue,
]);
const NONSTARTERS: Classes = Classes::new(&[
    L::BreakAfter,
    L::UnambiguousHyphen,
    L::Hyphen,
    L::Nonstarter,
]);
const IDEOGRAPHIC: Classes = Classes::new(&[L::Ideographic, L::EBase, L::EModifier]);
const AFFIXES: Classes = Classes::new(&[L::PrefixNumeric, L::PostfixNumeric]);
const NUMBER_STARTS: Classes = Classes::new(&[
    L::PrefixNumeric,
    L::PostfixNumeric,
    L::Hyphen,
    L::InfixNumeric,
]);
const NUMBER_SEPARATORS: Classes = Classes::new(&[L::BreakSymbols, L::InfixNumeric]);
const HANGUL: Classes = Classes::new(&[L::Jl, L::Jv, L::Jt, L::H2, L::H3]);
const HANGUL_LEADING: Classes = Classes::new(&[L::Jl, L::Jv, L::H2, L::H3]);
const HANGUL_VOWELS: Classes = Classes::new(&[L::Jv, L::H2]);
const HANGUL_VOWEL_FOLLOWERS: Classes = Classes::new(&[L::Jv, L::Jt]);
const HANGUL_TRAILS: Classes = Classes::new(&[L::Jt, L::H3]);
const VIRAMAS: Classes = Classes::new(&[L::ViramaFinal, L::Virama]);
const ALPHANUMERIC: Classes = Classes::new(&[L::Alphabetic, L::HebrewLetter, L::Numeric]);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Left {
    class: Class,
    opens_quote: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
enum Number {
    #[default]
    None,
    Open,
    Closed,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct Context {
    pub(crate) current: Option<Unit>,
    previous: Option<Unit>,
    left: Option<Left>,
    number: Number,
    odd_regional_indicators: bool,
}

#[derive(Debug)]
pub(crate) struct Lookahead<'a> {
    units: &'a [Option<Unit>],
    deepest: Cell<usize>,
}

impl<'a> Lookahead<'a> {
    pub(crate) fn new(units: &'a [Option<Unit>]) -> Self {
        Self {
            units,
            deepest: Cell::new(0),
        }
    }

    pub(crate) fn deepest(&self) -> usize {
        self.deepest.get()
    }

    fn unit(&self, depth: usize) -> Option<Unit> {
        self.deepest.set(self.deepest.get().max(depth + 1));
        self.units.get(depth).copied().flatten()
    }
}

impl Context {
    pub(crate) fn absorb(&self, kind: Unit) -> Option<Self> {
        let current = self.current?;
        (kind.attaches() && current.absorbs()).then(|| Self {
            current: Some(Unit {
                joiner_end: kind.class == L::Zwj,
                ..current
            }),
            ..*self
        })
    }

    pub(crate) fn advance(&self, next: Unit) -> Self {
        let mut context = Self {
            previous: self.current,
            current: Some(next),
            ..*self
        };
        if next.class != L::Space {
            context.left = Some(Left {
                class: next.class,
                opens_quote: next.initial_quote
                    && context
                        .previous
                        .is_none_or(|previous| QUOTE_OPENERS.has(previous.class)),
            });
        }
        context.number = match next.class {
            L::Numeric => Number::Open,
            _ if self.number != Number::Open => Number::None,
            class if NUMBER_SEPARATORS.has(class) => Number::Open,
            class if CLOSING.has(class) => Number::Closed,
            _ => Number::None,
        };
        context.odd_regional_indicators =
            next.class == L::RegionalIndicator && !self.odd_regional_indicators;
        context
    }

    pub(crate) fn opportunity(&self, next: &Unit, ahead: &Lookahead<'_>) -> Option<Decision> {
        let current = self.current?;
        let (left, right) = (current.class, next.class);
        match left {
            L::MandatoryBreak | L::LineFeed | L::NextLine => {
                return Some(Decision::Mandatory);
            }
            L::CarriageReturn if right != L::LineFeed => {
                return Some(Decision::Mandatory);
            }
            _ => {}
        }
        if HARD_BREAKS.has(right) || SPACES.has(right) {
            return None;
        }
        let before_spaces = self.left.map(|left| left.class);
        if before_spaces == Some(L::ZWSpace) {
            return Some(Decision::Allowed);
        }
        if current.joiner_end
            || right == L::WordJoiner
            || left == L::WordJoiner
            || left == L::Glue
            || (right == L::Glue && !GLUE_BREAKERS.has(left))
            || CLOSERS.has(right)
            || before_spaces == Some(L::OpenPunctuation)
            || self.left.is_some_and(|left| left.opens_quote)
        {
            return None;
        }
        if next.final_quote
            && ahead
                .unit(0)
                .is_none_or(|after| QUOTE_CLOSERS.has(after.class))
        {
            return None;
        }
        if left == L::Space
            && right == L::InfixNumeric
            && ahead.unit(0).is_some_and(|after| after.class == L::Numeric)
        {
            return Some(Decision::Allowed);
        }
        if right == L::InfixNumeric
            || (right == L::Nonstarter && before_spaces.is_some_and(|class| CLOSING.has(class)))
            || (right == L::BreakBoth && before_spaces == Some(L::BreakBoth))
        {
            return None;
        }
        if left == L::Space {
            return Some(Decision::Allowed);
        }
        if self.quotation_glues(&current, next, ahead) {
            return None;
        }
        if left == L::ContingentBreak || right == L::ContingentBreak {
            return Some(Decision::Allowed);
        }
        let previous = self.previous.as_ref();
        if HYPHENS.has(left)
            && LETTERS.has(right)
            && previous.is_none_or(|previous| WORD_STARTS.has(previous.class))
        {
            return None;
        }
        if NONSTARTERS.has(right) || left == L::BreakBefore {
            return None;
        }
        if HYPHENS.has(left)
            && right != L::HebrewLetter
            && previous.is_some_and(|previous| previous.class == L::HebrewLetter)
        {
            return None;
        }
        if (left == L::BreakSymbols && right == L::HebrewLetter) || right == L::Inseparable {
            return None;
        }
        if self.alphanumerics_glue(&current, next, ahead) {
            return None;
        }
        Some(Decision::Allowed)
    }

    fn quotation_glues(&self, current: &Unit, next: &Unit, ahead: &Lookahead<'_>) -> bool {
        let (left, right) = (current.class, next.class);
        if right == L::Quotation
            && (!next.initial_quote
                || !current.east_asian
                || !ahead.unit(0).is_some_and(|after| after.east_asian))
        {
            return true;
        }
        left == L::Quotation
            && (!current.final_quote
                || !next.east_asian
                || !self.previous.is_some_and(|previous| previous.east_asian))
    }

    fn alphanumerics_glue(&self, current: &Unit, next: &Unit, ahead: &Lookahead<'_>) -> bool {
        let (left, right) = (current.class, next.class);
        let after_class = || ahead.unit(0).map(|after| after.class);
        let beyond_class = || ahead.unit(1).map(|beyond| beyond.class);
        (LETTERS.has(left) && right == L::Numeric)
            || (left == L::Numeric && LETTERS.has(right))
            || (left == L::PrefixNumeric && IDEOGRAPHIC.has(right))
            || (IDEOGRAPHIC.has(left) && right == L::PostfixNumeric)
            || (AFFIXES.has(left) && LETTERS.has(right))
            || (LETTERS.has(left) && AFFIXES.has(right))
            || (self.number != Number::None && AFFIXES.has(right))
            || (AFFIXES.has(left)
                && right == L::OpenPunctuation
                && (after_class() == Some(L::Numeric)
                    || (after_class() == Some(L::InfixNumeric)
                        && beyond_class() == Some(L::Numeric))))
            || (NUMBER_STARTS.has(left) && right == L::Numeric)
            || (self.number == Number::Open && right == L::Numeric)
            || (left == L::Jl && HANGUL_LEADING.has(right))
            || (HANGUL_VOWELS.has(left) && HANGUL_VOWEL_FOLLOWERS.has(right))
            || (HANGUL_TRAILS.has(left) && right == L::Jt)
            || (HANGUL.has(left) && right == L::PostfixNumeric)
            || (left == L::PrefixNumeric && HANGUL.has(right))
            || (LETTERS.has(left) && LETTERS.has(right))
            || self.aksaras_glue(current, next, ahead)
            || (left == L::InfixNumeric && LETTERS.has(right))
            || (ALPHANUMERIC.has(left) && right == L::OpenPunctuation && !next.east_asian)
            || (left == L::CloseParenthesis && !current.east_asian && ALPHANUMERIC.has(right))
            || (left == L::RegionalIndicator
                && right == L::RegionalIndicator
                && self.odd_regional_indicators)
            || (right == L::EModifier && (left == L::EBase || current.unassigned_pictographic))
    }

    fn aksaras_glue(&self, current: &Unit, next: &Unit, ahead: &Lookahead<'_>) -> bool {
        (current.class == L::AksaraPrebase && next.is_aksara())
            || (current.is_aksara() && VIRAMAS.has(next.class))
            || (current.class == L::Virama
                && self.previous.is_some_and(|previous| previous.is_aksara())
                && (next.class == L::Aksara || next.dotted_circle))
            || (current.is_aksara()
                && next.is_aksara()
                && ahead
                    .unit(0)
                    .is_some_and(|after| after.class == L::ViramaFinal))
    }
}

use std::fmt;

use crate::args::{Args, Value};
use crate::plural::{PluralCategory, PluralKind, PluralOperands};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Part {
    Literal(Box<str>),
    Arg(Box<str>),
    Hash,
    Plural {
        arg: Box<str>,
        kind: PluralKind,
        arms: Vec<PluralArm>,
    },
    Select {
        arg: Box<str>,
        arms: Vec<SelectArm>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PluralArm {
    pub(crate) selector: Selector,
    pub(crate) body: Pattern,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SelectArm {
    pub(crate) keyword: Box<str>,
    pub(crate) body: Pattern,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Selector {
    Exact(i64),
    Category(PluralCategory),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Pattern {
    parts: Vec<Part>,
}

impl Pattern {
    const EMPTY: &'static Self = &Self { parts: Vec::new() };

    pub(crate) fn new(parts: Vec<Part>) -> Self {
        Self { parts }
    }

    pub(crate) fn as_literal(&self) -> Option<&str> {
        match self.parts.as_slice() {
            [] => Some(""),
            [Part::Literal(text)] => Some(text),
            _ => None,
        }
    }

    pub(crate) fn format(
        &self,
        language: &str,
        args: &Args<'_>,
        hash: Option<Value<'_>>,
        out: &mut impl fmt::Write,
    ) -> fmt::Result {
        for part in &self.parts {
            match part {
                Part::Literal(text) => out.write_str(text)?,
                Part::Hash => match hash {
                    Some(value) => write!(out, "{value}")?,
                    None => out.write_char('#')?,
                },
                Part::Arg(name) => match args.get(name) {
                    Some(value) => write!(out, "{value}")?,
                    None => write!(out, "{{{name}}}")?,
                },
                Part::Plural { arg, kind, arms } => match args.get(arg) {
                    Some(value) => select_plural(language, *kind, value, arms).format(
                        language,
                        args,
                        Some(value),
                        out,
                    )?,
                    None => write!(out, "{{{arg}}}")?,
                },
                Part::Select { arg, arms } => match args.get(arg) {
                    Some(value) => select_keyword(&value.to_string(), arms)
                        .format(language, args, hash, out)?,
                    None => write!(out, "{{{arg}}}")?,
                },
            }
        }
        Ok(())
    }
}

fn select_plural<'a>(
    language: &str,
    kind: PluralKind,
    value: Value<'_>,
    arms: &'a [PluralArm],
) -> &'a Pattern {
    let operands = PluralOperands::from_value(value);
    if let Value::Int(number) = value
        && let Some(arm) = arms
            .iter()
            .find(|arm| arm.selector == Selector::Exact(number))
    {
        return &arm.body;
    }
    let category = operands
        .map(|operands| PluralCategory::select(language, kind, operands))
        .unwrap_or(PluralCategory::Other);

    arm_body(arms, Selector::Category(category))
        .or_else(|| arm_body(arms, Selector::Category(PluralCategory::Other)))
        .unwrap_or(Pattern::EMPTY)
}

fn arm_body(arms: &[PluralArm], selector: Selector) -> Option<&Pattern> {
    arms.iter()
        .find(|arm| arm.selector == selector)
        .map(|arm| &arm.body)
}

fn select_keyword<'a>(value: &str, arms: &'a [SelectArm]) -> &'a Pattern {
    let find = |keyword: &str| {
        arms.iter()
            .find(|arm| &*arm.keyword == keyword)
            .map(|arm| &arm.body)
    };
    find(value)
        .or_else(|| find("other"))
        .unwrap_or(Pattern::EMPTY)
}

use std::ops::RangeInclusive;

use libspa::pod::{ChoiceValue, Value};
use libspa::utils::{Choice, ChoiceEnum};

pub const RATES: [u32; 15] = [
    8_000, 11_025, 16_000, 22_050, 32_000, 44_100, 48_000, 88_200, 96_000, 176_400, 192_000,
    352_800, 384_000, 705_600, 768_000,
];

#[must_use]
pub fn rates_within(span: &RangeInclusive<u32>) -> Vec<u32> {
    RATES
        .into_iter()
        .filter(|rate| span.contains(rate))
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OfferedRates {
    Listed(Vec<u32>),
    Span(RangeInclusive<u32>),
}

impl OfferedRates {
    #[must_use]
    pub fn parse(value: &Value) -> Option<OfferedRates> {
        let positive = |value: &i32| u32::try_from(*value).ok().filter(|value| *value > 0);
        match value {
            Value::Int(value) => Some(OfferedRates::Listed(positive(value).into_iter().collect())),
            Value::Choice(ChoiceValue::Int(Choice(_, choice))) => match choice {
                ChoiceEnum::None(value) => {
                    Some(OfferedRates::Listed(positive(value).into_iter().collect()))
                }
                ChoiceEnum::Range { min, max, .. } | ChoiceEnum::Step { min, max, .. } => {
                    Some(OfferedRates::Span(positive(min)?..=positive(max)?))
                }
                ChoiceEnum::Enum {
                    default,
                    alternatives,
                } => Some(OfferedRates::Listed(
                    std::iter::once(default)
                        .chain(alternatives)
                        .filter_map(positive)
                        .collect(),
                )),
                ChoiceEnum::Flags { .. } => None,
            },
            _ => None,
        }
    }

    #[must_use]
    pub fn span(&self) -> Option<RangeInclusive<u32>> {
        match self {
            OfferedRates::Listed(values) => Some(*values.iter().min()?..=*values.iter().max()?),
            OfferedRates::Span(span) => Some(span.clone()),
        }
    }

    #[must_use]
    pub fn rates(&self) -> Vec<u32> {
        match self {
            OfferedRates::Listed(values) => values.clone(),
            OfferedRates::Span(span) => rates_within(span),
        }
    }
}

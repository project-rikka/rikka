use std::ops::RangeInclusive;

use rikka_pipewire::RATES;

use crate::format::SampleFormat;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hardware {
    pub formats: Vec<SampleFormat>,
    pub rates: Option<RangeInclusive<u32>>,
    pub refuses_pinned_format: bool,
}

impl Hardware {
    pub fn plays(&self, format: SampleFormat) -> bool {
        self.formats.contains(&format)
    }

    pub fn takes(&self, rate: u32) -> bool {
        self.rates
            .as_ref()
            .is_some_and(|rates| rates.contains(&rate))
    }

    pub fn rates(&self) -> impl Iterator<Item = u32> + '_ {
        RATES.into_iter().filter(|rate| self.takes(*rate))
    }

    pub(crate) fn widened(&self, observed: &Hardware) -> Hardware {
        let rates = match (&self.rates, &observed.rates) {
            (Some(known), Some(observed)) => {
                Some(*known.start().min(observed.start())..=*known.end().max(observed.end()))
            }
            (known, observed) => known.clone().or_else(|| observed.clone()),
        };
        Hardware {
            formats: SampleFormat::ALL
                .into_iter()
                .filter(|format| self.plays(*format) || observed.plays(*format))
                .collect(),
            rates,
            refuses_pinned_format: false,
        }
    }
}

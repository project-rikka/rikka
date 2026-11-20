use crate::sample_format::SampleFormat;

pub const DSD_RATES: [u32; 4] = [2_822_400, 5_644_800, 11_289_600, 22_579_200];

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Capabilities {
    pub formats: Vec<SampleFormat>,
    pub rates: Vec<u32>,
    pub channels: Vec<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DsdSupport {
    pub native: Vec<u32>,
    pub dop: Vec<u32>,
}

impl Capabilities {
    #[must_use]
    pub fn offers(&self, format: SampleFormat) -> bool {
        self.formats.contains(&format)
    }

    #[must_use]
    pub fn runs_at(&self, rate: u32) -> bool {
        self.rates.contains(&rate)
    }

    #[must_use]
    pub fn native_dsd(&self, dsd_rate: u32) -> Option<SampleFormat> {
        SampleFormat::DSD
            .into_iter()
            .find(|format| self.offers(*format) && self.runs_at(dsd_word_rate(dsd_rate, *format)))
    }

    #[must_use]
    pub fn dop_container(&self, dsd_rate: u32) -> Option<SampleFormat> {
        if !self.runs_at(dsd_rate / 16) {
            return None;
        }
        SampleFormat::INTEGER
            .into_iter()
            .find(|format| format.carries_dop() && self.offers(*format))
    }

    #[must_use]
    pub fn dsd(&self) -> DsdSupport {
        DsdSupport {
            native: DSD_RATES
                .into_iter()
                .filter(|rate| self.native_dsd(*rate).is_some())
                .collect(),
            dop: DSD_RATES
                .into_iter()
                .filter(|rate| self.dop_container(*rate).is_some())
                .collect(),
        }
    }

    #[must_use]
    pub fn best_pcm_format(&self) -> Option<SampleFormat> {
        SampleFormat::INTEGER
            .into_iter()
            .chain([SampleFormat::F32])
            .find(|format| self.offers(*format))
    }

    #[must_use]
    pub fn closest_rate(&self, target: u32) -> Option<u32> {
        if self.runs_at(target) {
            return Some(target);
        }
        let above = || self.rates.iter().copied().filter(|rate| *rate > target);
        above()
            .filter(|rate| rate % target == 0)
            .min()
            .or_else(|| above().min())
            .or_else(|| {
                self.rates
                    .iter()
                    .copied()
                    .filter(|rate| *rate < target)
                    .max()
            })
    }

    #[must_use]
    pub fn closest_channels(&self, target: u16) -> Option<u16> {
        if self.channels.contains(&target) {
            return Some(target);
        }
        let above = self
            .channels
            .iter()
            .copied()
            .filter(|count| *count > target)
            .min();
        above.or_else(|| self.channels.iter().copied().max())
    }
}

#[must_use]
pub fn dsd_word_rate(dsd_rate: u32, format: SampleFormat) -> u32 {
    dsd_rate / (8 * format.bytes() as u32)
}

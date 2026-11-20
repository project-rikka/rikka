use crate::capabilities::{Capabilities, dsd_word_rate};
use crate::dsd_pcm::DsdToPcm;
use crate::sample_format::SampleFormat;
use crate::source_format::{Resolution, SourceFormat};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Carriage {
    Pcm,
    DsdToPcm,
    Dop,
    NativeDsd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Plan {
    pub rate: u32,
    pub channels: u16,
    pub format: SampleFormat,
    pub carriage: Carriage,
}

impl Plan {
    #[must_use]
    pub fn shared(source: &SourceFormat) -> Plan {
        let (rate, carriage) = match source.resolution {
            Resolution::Pcm { rate, .. } => (rate, Carriage::Pcm),
            Resolution::Dsd { rate } => (DsdToPcm::pcm_rate(rate), Carriage::DsdToPcm),
        };
        Plan {
            rate,
            channels: source.channels,
            format: SampleFormat::F32,
            carriage,
        }
    }

    #[must_use]
    pub fn exclusive(source: &SourceFormat, device: &Capabilities) -> Option<Plan> {
        match source.resolution {
            Resolution::Pcm { rate, .. } => pcm(rate, source.channels, device, Carriage::Pcm),
            Resolution::Dsd { rate } => dsd(rate, source.channels, device),
        }
    }

    #[must_use]
    pub fn frame_bytes(&self) -> usize {
        self.format.bytes() * usize::from(self.channels)
    }
}

fn pcm(rate: u32, channels: u16, device: &Capabilities, carriage: Carriage) -> Option<Plan> {
    Some(Plan {
        rate: device.closest_rate(rate)?,
        channels: device.closest_channels(channels)?,
        format: device.best_pcm_format()?,
        carriage,
    })
}

fn dsd(rate: u32, channels: u16, device: &Capabilities) -> Option<Plan> {
    let exact_channels = device.channels.contains(&channels);
    if exact_channels && let Some(format) = device.native_dsd(rate) {
        return Some(Plan {
            rate: dsd_word_rate(rate, format),
            channels,
            format,
            carriage: Carriage::NativeDsd,
        });
    }
    if exact_channels && let Some(format) = device.dop_container(rate) {
        return Some(Plan {
            rate: rate / 16,
            channels,
            format,
            carriage: Carriage::Dop,
        });
    }
    pcm(
        DsdToPcm::pcm_rate(rate),
        channels,
        device,
        Carriage::DsdToPcm,
    )
}

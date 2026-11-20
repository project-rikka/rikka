use libspa::pod::Pod;
use libspa::sys;
use rikka_hifi_core::SampleFormat;
use rikka_pipewire::{Fields, OfferedRates, RawFormat, Sample};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Negotiated {
    pub(super) rate: u32,
    pub(super) channels: u16,
    pub(super) format: Option<SampleFormat>,
}

pub(super) fn negotiated(param: &Pod) -> Option<Negotiated> {
    let raw = RawFormat::parse(param)?;
    (raw.rate > 0).then(|| Negotiated {
        rate: raw.rate,
        channels: u16::try_from(raw.channels).unwrap_or(u16::MAX),
        format: raw.sample.map(sample_format),
    })
}

fn sample_format(sample: Sample) -> SampleFormat {
    match sample {
        Sample::S16 => SampleFormat::S16,
        Sample::S24 => SampleFormat::S24Packed,
        Sample::S24In32 => SampleFormat::S24,
        Sample::S32 => SampleFormat::S32,
        Sample::F32 => SampleFormat::F32,
        Sample::F64 => SampleFormat::F64,
    }
}

pub(super) fn soft_volume(param: &Pod) -> Option<f32> {
    let fields = Fields::parse(param)?;
    if fields.bool(sys::SPA_PROP_softMute) == Some(true) {
        return Some(0.0);
    }
    fields
        .floats(sys::SPA_PROP_softVolumes)?
        .iter()
        .copied()
        .reduce(f32::max)
}

pub(super) fn offered_rates(param: &Pod) -> Vec<u32> {
    Fields::parse(param)
        .and_then(|fields| {
            fields
                .get(sys::SPA_FORMAT_AUDIO_rate)
                .and_then(OfferedRates::parse)
        })
        .map(|offered| offered.rates())
        .unwrap_or_default()
}

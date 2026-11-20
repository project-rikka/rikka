use rikka_hifi_core::Capabilities;
use rikka_pipewire::RATES;

use crate::exclusive::descriptors;
use crate::exclusive::formats::{self, PLAYABLE};
use crate::exclusive::refine::Substream;

const MAX_CHANNELS: u32 = 8;

pub(crate) fn capabilities(card: u32, device: u32) -> Option<Capabilities> {
    match descriptors::usb(card, device) {
        Some(described) => descriptors::capabilities(&described),
        None => refined(card, device),
    }
}

pub(crate) fn card_rates(card: u32, device: u32) -> Option<Vec<u32>> {
    capabilities(card, device).map(|capabilities| capabilities.rates)
}

fn refined(card: u32, device: u32) -> Option<Capabilities> {
    let substream = match Substream::open(card, device) {
        Ok(substream) => substream,
        Err(error) => {
            tracing::debug!("failed to probe card {card} device {device}: {error}");
            return None;
        }
    };
    let space = substream.interleaved()?;
    let formats = PLAYABLE
        .into_iter()
        .filter(|format| substream.contains(space.with_format(formats::alsa(*format) as u32)))
        .collect();
    let rates = RATES
        .into_iter()
        .filter(|rate| substream.contains(space.with_rate(*rate)))
        .collect();
    let (lowest, highest) = space.channels();
    let channels = (lowest..=highest.min(MAX_CHANNELS))
        .filter(|count| substream.contains(space.with_channels(*count)))
        .filter_map(|count| u16::try_from(count).ok())
        .collect();
    Some(Capabilities {
        formats,
        rates,
        channels,
    })
}

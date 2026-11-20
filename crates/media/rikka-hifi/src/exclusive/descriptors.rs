use rikka_hifi_core::{Capabilities, SampleFormat};
use rikka_pipewire::rates_within;

use crate::exclusive::formats;

pub(crate) fn usb(card: u32, device: u32) -> Option<String> {
    std::fs::read_to_string(format!("/proc/asound/card{card}/stream{device}")).ok()
}

pub(crate) fn capabilities(described: &str) -> Option<Capabilities> {
    let playback = described
        .split("Capture:")
        .next()?
        .split_once("Playback:")?
        .1;
    let mut found = Capabilities::default();
    for line in playback.lines().map(str::trim) {
        if let Some(names) = line.strip_prefix("Format:") {
            for format in names.split_whitespace().filter_map(format) {
                if !found.formats.contains(&format) {
                    found.formats.push(format);
                }
            }
        } else if let Some(count) = line.strip_prefix("Channels:") {
            if let Ok(count) = count.trim().parse()
                && !found.channels.contains(&count)
            {
                found.channels.push(count);
            }
        } else if let Some(rates) = line.strip_prefix("Rates:") {
            for rate in rates_in(rates) {
                if !found.rates.contains(&rate) {
                    found.rates.push(rate);
                }
            }
        }
    }
    found.rates.sort_unstable();
    found.channels.sort_unstable();
    (!found.formats.is_empty() && !found.rates.is_empty()).then_some(found)
}

fn format(name: &str) -> Option<SampleFormat> {
    match name {
        "FLOAT_LE" => Some(SampleFormat::F32),
        "FLOAT64_LE" => Some(SampleFormat::F64),
        other => formats::named(other),
    }
}

fn rates_in(text: &str) -> Vec<u32> {
    let numbers = rikka_pipewire::numbers(text);
    if text.contains("continuous")
        && let [lowest, highest, ..] = numbers[..]
    {
        return rates_within(&(lowest..=highest));
    }
    numbers
}

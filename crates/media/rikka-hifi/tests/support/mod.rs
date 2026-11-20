pub(crate) mod card;

use std::time::{Duration, Instant};

use async_io::Timer;
use futures_util::StreamExt as _;
use futures_util::future::{Either, select};
use rikka_hifi::{Event, Hifi, Identity, Source, Verdict};
use rikka_worker::Subscription;

pub(crate) const PATIENCE: Duration = Duration::from_secs(10);

pub(crate) fn hifi() -> Hifi {
    hifi_as("Rikka Hifi Test")
}

pub(crate) fn hifi_as(application: &str) -> Hifi {
    Hifi::start(Identity::new(application, "rikka-hifi-test").icon("audio-x-generic"))
        .expect("the hifi player starts")
}

pub(crate) fn tone(rate: u32, seconds: f32, frequency: f32) -> Source {
    let frames = (rate as f32 * seconds) as usize;
    let mut wav = Vec::with_capacity(44 + frames * 4);
    let data = (frames * 4) as u32;
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&2_u16.to_le_bytes());
    wav.extend_from_slice(&rate.to_le_bytes());
    wav.extend_from_slice(&(rate * 4).to_le_bytes());
    wav.extend_from_slice(&4_u16.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data.to_le_bytes());
    for frame in 0..frames {
        let phase = std::f32::consts::TAU * frequency * frame as f32 / rate as f32;
        let value = (phase.sin() * 3000.0) as i16;
        wav.extend_from_slice(&value.to_le_bytes());
        wav.extend_from_slice(&value.to_le_bytes());
    }
    let decoder = rikka_audio_decode::read(wav)
        .decoder()
        .expect("the tone decodes");
    Source::decoder(decoder)
}

pub(crate) const DSD64: u32 = 2_822_400;

pub(crate) fn dsd() -> Source {
    Source::dsd(
        rikka_audio_decode::read(dsf(1))
            .dsd()
            .expect("the DSF opens"),
    )
}

fn dsf(seconds: u32) -> Vec<u8> {
    const BLOCK: usize = 4096;
    let frames = u64::from(DSD64 * seconds);
    let per_channel = (frames as usize / 8).div_ceil(BLOCK) * BLOCK;
    let data = per_channel * 2;
    let mut dsf = Vec::with_capacity(92 + data);
    dsf.extend_from_slice(b"DSD ");
    dsf.extend_from_slice(&28_u64.to_le_bytes());
    dsf.extend_from_slice(&((92 + data) as u64).to_le_bytes());
    dsf.extend_from_slice(&0_u64.to_le_bytes());
    dsf.extend_from_slice(b"fmt ");
    dsf.extend_from_slice(&52_u64.to_le_bytes());
    for field in [1_u32, 0, 2, 2, DSD64, 1] {
        dsf.extend_from_slice(&field.to_le_bytes());
    }
    dsf.extend_from_slice(&frames.to_le_bytes());
    dsf.extend_from_slice(&(BLOCK as u32).to_le_bytes());
    dsf.extend_from_slice(&0_u32.to_le_bytes());
    dsf.extend_from_slice(b"data");
    dsf.extend_from_slice(&((12 + data) as u64).to_le_bytes());
    dsf.resize(92 + data, 0x69);
    dsf
}

pub(crate) fn next(events: &mut Subscription<Event>, until: Instant) -> Option<Event> {
    async_io::block_on(async {
        match select(events.next(), Timer::at(until)).await {
            Either::Left((event, _)) => event,
            Either::Right(_) => None,
        }
    })
}

pub(crate) fn wait_for(
    events: &mut Subscription<Event>,
    seen: &mut Vec<Event>,
    mut matches: impl FnMut(&Event) -> bool,
) -> Option<Event> {
    let until = Instant::now() + PATIENCE;
    while let Some(event) = next(events, until) {
        seen.push(event.clone());
        if matches(&event) {
            return Some(event);
        }
    }
    None
}

pub(crate) fn describe(seen: &[Event]) {
    for event in seen {
        match event {
            Event::Position { .. } => {}
            Event::Path(path) => eprintln!("path ({:?}): {path:#?}", path.verdict()),
            other => eprintln!("{other:?}"),
        }
    }
}

pub(crate) fn last_verdict(seen: &[Event]) -> Option<Verdict> {
    seen.iter().rev().find_map(|event| match event {
        Event::Path(path) => Some(path.verdict()),
        _ => None,
    })
}

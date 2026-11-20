use std::ops::ControlFlow;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use music_core::{Deck, Events, PlaybackEvent};
use rikka_hifi::{Hifi, Identity};

pub(crate) const PATIENCE: Duration = Duration::from_secs(10);

pub(crate) fn deck() -> Deck {
    let hifi = Hifi::start(Identity::new("Rikka Music Test", "rikka-music-test"))
        .expect("the hifi player starts");
    hifi.set_volume(0.05);
    Deck::new(hifi)
}

pub(crate) fn wav(rate: u32, frames: u32) -> Vec<u8> {
    let data = frames * 4;
    let mut wav = Vec::with_capacity(44 + data as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&rate.to_le_bytes());
    wav.extend_from_slice(&(rate * 4).to_le_bytes());
    wav.extend_from_slice(&4u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data.to_le_bytes());
    wav.resize(44 + data as usize, 0);
    wav
}

pub(crate) fn forwarded(events: Events) -> Receiver<PlaybackEvent> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        music_core::drain(events, |event| match sender.send(event) {
            Ok(()) => ControlFlow::Continue(()),
            Err(_) => ControlFlow::Break(()),
        });
    });
    receiver
}

pub(crate) fn wait_for(
    events: &Receiver<PlaybackEvent>,
    seen: &mut Vec<PlaybackEvent>,
    mut matches: impl FnMut(&PlaybackEvent) -> bool,
) -> Option<PlaybackEvent> {
    let deadline = Instant::now() + PATIENCE;
    loop {
        let event = events
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .ok()?;
        seen.push(event.clone());
        if matches(&event) {
            return Some(event);
        }
    }
}

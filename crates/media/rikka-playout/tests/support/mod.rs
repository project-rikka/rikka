use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use rikka_playout::{CHANNELS, CHUNK, Decode, FRAME, Packet, Playout, RATE, SpeechType};

mod device;

pub(crate) use device::{Device, simulate_device};

pub(crate) const TONE_HZ: f32 = 300.0;

pub(crate) const BASE: u32 = 0x7fff_f000;

pub(crate) fn silent_tail(packets: u32) -> Option<u32> {
    Some(BASE + (packets - 5) * FRAME as u32)
}

#[derive(Default)]
pub(crate) struct Counts {
    pub(crate) decoded: Cell<u32>,
    pub(crate) recovered: Cell<u32>,
    pub(crate) concealed: Cell<u32>,
}

pub(crate) struct Tone {
    pub(crate) counts: Rc<Counts>,
    pub(crate) silent_from: Option<u32>,
}

pub(crate) fn sample(timestamp: u32) -> f32 {
    let phase = (timestamp % 4_800) as f32;
    0.5 * (std::f32::consts::TAU * TONE_HZ * phase / RATE as f32).sin()
}

fn render(timestamp: u32, frames: usize, silent: bool, out: &mut Vec<f32>) {
    for offset in 0..frames {
        let value = if silent {
            0.0
        } else {
            sample(timestamp.wrapping_add(offset as u32))
        };
        out.extend(std::iter::repeat_n(value, CHANNELS));
    }
}

impl Decode for Tone {
    fn frames(&self, _: &[u8]) -> Option<usize> {
        Some(FRAME)
    }

    fn decode(&mut self, packet: &[u8], out: &mut Vec<f32>) -> bool {
        self.counts.decoded.set(self.counts.decoded.get() + 1);
        let timestamp = u32::from_le_bytes(packet[..4].try_into().unwrap());
        let silent = self.silent_from.is_some_and(|from| timestamp >= from);
        render(timestamp, FRAME, silent, out);
        true
    }

    fn recover(&mut self, next: &[u8], frames: usize, out: &mut Vec<f32>) -> bool {
        self.counts.recovered.set(self.counts.recovered.get() + 1);
        let timestamp = u32::from_le_bytes(next[..4].try_into().unwrap());
        render(timestamp.wrapping_sub(frames as u32), frames, false, out);
        true
    }

    fn conceal(&mut self, frames: usize, out: &mut Vec<f32>) {
        self.counts.concealed.set(self.counts.concealed.get() + 1);
        out.extend(std::iter::repeat_n(0.0, frames * CHANNELS));
    }
}

pub(crate) struct Network {
    pub(crate) delay: Box<dyn FnMut(u32) -> Duration>,
    pub(crate) lost: Box<dyn FnMut(u32) -> bool>,
    pub(crate) sender_speed: f64,
}

impl Network {
    pub(crate) fn steady() -> Network {
        Network {
            delay: Box::new(|_| Duration::from_millis(40)),
            lost: Box::new(|_| false),
            sender_speed: 1.0,
        }
    }
}

pub(crate) struct Run {
    pub(crate) speech: Vec<SpeechType>,
    pub(crate) output: Vec<f32>,
    pub(crate) targets: Vec<Duration>,
    pub(crate) buffered: Vec<Duration>,
    pub(crate) counts: Rc<Counts>,
}

pub(crate) fn simulate(
    mut network: Network,
    packets: u32,
    silent_from: Option<u32>,
    seconds: f64,
) -> Run {
    let start = Instant::now();
    let counts = Rc::new(Counts::default());
    let mut playout = Playout::new(
        Tone {
            counts: Rc::clone(&counts),
            silent_from,
        },
        start,
    );
    let base = BASE;
    let mut arrivals: Vec<(Duration, Packet)> = (0..packets)
        .filter(|index| !(network.lost)(*index))
        .map(|index| {
            let sent = Duration::from_secs_f64(index as f64 * 0.02 / network.sender_speed);
            let timestamp = base.wrapping_add(index * FRAME as u32);
            let arrival = sent + (network.delay)(index);
            let packet = Packet {
                timestamp,
                arrived: start + arrival,
                payload: timestamp.to_le_bytes().to_vec(),
            };
            (arrival, packet)
        })
        .collect();
    arrivals.sort_by_key(|(at, _)| *at);
    let mut arrivals = arrivals.into_iter().peekable();
    let mut run = Run {
        speech: Vec::new(),
        output: Vec::new(),
        targets: Vec::new(),
        buffered: Vec::new(),
        counts,
    };
    let mut chunk = vec![0.0; CHUNK * CHANNELS];
    let ticks = (seconds * 100.0) as u32;
    for tick in 0..ticks {
        let now = Duration::from_millis(u64::from(tick) * 10);
        while arrivals.peek().is_some_and(|(at, _)| *at <= now) {
            let (_, packet) = arrivals.next().unwrap();
            playout.push(packet);
        }
        run.speech.push(playout.pull(start + now, &mut chunk));
        run.output.extend(chunk.iter().step_by(CHANNELS));
        run.targets.push(playout.target());
        run.buffered.push(playout.buffered());
    }
    run
}

pub(crate) fn count_glitches(output: &[f32]) -> usize {
    let step = std::f32::consts::TAU * TONE_HZ / RATE as f32 * 0.5;
    output
        .windows(2)
        .filter(|pair| pair[0] != 0.0 && pair[1] != 0.0)
        .filter(|pair| (pair[1] - pair[0]).abs() > step * 1.6)
        .count()
}

pub(crate) fn speech_span(speech: &[SpeechType]) -> &[SpeechType] {
    let first = speech
        .iter()
        .position(|kind| *kind == SpeechType::Speech)
        .unwrap_or(0);
    let last = speech
        .iter()
        .rposition(|kind| *kind == SpeechType::Speech)
        .unwrap_or(0);
    &speech[first..=last]
}

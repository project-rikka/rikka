use std::collections::VecDeque;
use std::rc::Rc;
use std::time::{Duration, Instant};

use rikka_playout::{CHANNELS, CHUNK, FRAME, Packet, Playout, QueueTarget, RATE, SpeechType};

use super::{BASE, Counts, Network, Tone};

const BURST: u32 = 8;

pub(crate) struct Device {
    pub(crate) quantum: usize,
    pub(crate) poll: Duration,
}

impl Device {
    pub(crate) fn with_quantum(quantum: usize) -> Device {
        Device {
            quantum,
            poll: Duration::from_millis(4),
        }
    }
}

pub(crate) struct DeviceRun {
    pub(crate) speech: Vec<SpeechType>,
    pub(crate) played: Vec<f32>,
    pub(crate) underruns: Vec<Duration>,
    pub(crate) counts: Rc<Counts>,
}

pub(crate) fn simulate_device(
    mut network: Network,
    device: Device,
    packets: u32,
    silent_from: Option<u32>,
    seconds: f64,
) -> DeviceRun {
    let start = Instant::now();
    let counts = Rc::new(Counts::default());
    let mut playout = Playout::new(
        Tone {
            counts: Rc::clone(&counts),
            silent_from,
        },
        start,
    );
    let mut arrivals: Vec<(Duration, Packet)> = (0..packets)
        .filter(|index| !(network.lost)(*index))
        .map(|index| {
            let sent = Duration::from_secs_f64(f64::from(index) * 0.02 / network.sender_speed);
            let timestamp = BASE.wrapping_add(index * FRAME as u32);
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
    let mut run = DeviceRun {
        speech: Vec::new(),
        played: Vec::new(),
        underruns: Vec::new(),
        counts,
    };
    let mut ring: VecDeque<f32> = VecDeque::new();
    let mut chunk = vec![0.0; CHUNK * CHANNELS];
    let quantum_time = frames_time(device.quantum);
    let chunk_time = frames_time(CHUNK);
    let mut queue_target = QueueTarget::new();
    let mut next_callback = quantum_time;
    let mut next_poll = Duration::ZERO;
    let end = Duration::from_secs_f64(seconds);
    let mut started = false;
    while next_poll < end || next_callback < end {
        if next_callback <= next_poll {
            let now = next_callback;
            next_callback += quantum_time;
            if !started {
                continue;
            }
            if ring.len() < device.quantum {
                run.underruns.push(now);
            }
            let count = device.quantum.min(ring.len());
            run.played.extend(ring.drain(..count));
            continue;
        }
        let now = next_poll;
        next_poll += device.poll;
        while arrivals.peek().is_some_and(|(at, _)| *at <= now) {
            let (_, packet) = arrivals.next().unwrap();
            playout.push(packet);
        }
        queue_target.observe(
            start + now,
            frames_time(ring.len()),
            run.underruns.len() as u32,
        );
        let mut produced = 0;
        while frames_time(ring.len()) < queue_target.target() && produced < BURST {
            run.speech.push(playout.pull(start + now, &mut chunk));
            ring.extend(chunk.iter().step_by(CHANNELS));
            queue_target.wrote(chunk_time);
            produced += 1;
            started = true;
        }
    }
    run
}

fn frames_time(frames: usize) -> Duration {
    Duration::from_secs_f64(frames as f64 / f64::from(RATE))
}

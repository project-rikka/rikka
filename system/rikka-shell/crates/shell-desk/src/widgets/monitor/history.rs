use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rikka_sysmon::Vitals;

pub(super) const SAMPLES: usize = 60;

const GAP: Duration = Duration::from_secs(5);
const QUIET_RATE: f32 = 64.0 * 1024.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Trace {
    Cpu,
    Memory,
    Gpu,
    Receive,
    Send,
    Io,
}

const TRACES: [Trace; 6] = [
    Trace::Cpu,
    Trace::Memory,
    Trace::Gpu,
    Trace::Receive,
    Trace::Send,
    Trace::Io,
];

#[derive(Debug, Default)]
pub(crate) struct History {
    vitals: Option<Arc<Vitals>>,
    at: Option<Instant>,
    traces: [VecDeque<f32>; TRACES.len()],
}

impl History {
    #[must_use]
    pub(crate) fn vitals(&self) -> Option<&Vitals> {
        self.vitals.as_deref()
    }

    pub(crate) fn record(&mut self, vitals: Arc<Vitals>) -> bool {
        let latest = self
            .vitals
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, &vitals));
        if latest || vitals.memory.total == 0 {
            return false;
        }
        let now = Instant::now();
        if self.at.is_some_and(|at| now.duration_since(at) > GAP) {
            self.traces.iter_mut().for_each(VecDeque::clear);
        }
        self.at = Some(now);
        for (trace, samples) in TRACES.iter().zip(&mut self.traces) {
            if samples.len() == SAMPLES {
                samples.pop_front();
            }
            samples.push_back(reading(*trace, &vitals));
        }
        self.vitals = Some(vitals);
        true
    }

    #[must_use]
    pub(crate) fn trace(&self, trace: Trace) -> Vec<f32> {
        let samples = &self.traces[trace as usize];
        match trace {
            Trace::Cpu | Trace::Memory | Trace::Gpu => samples.iter().copied().collect(),
            Trace::Receive | Trace::Send | Trace::Io => {
                let top = samples.iter().copied().fold(QUIET_RATE, f32::max);
                samples.iter().map(|rate| rate / top).collect()
            }
        }
    }
}

fn reading(trace: Trace, vitals: &Vitals) -> f32 {
    match trace {
        Trace::Cpu => vitals.cpu.usage,
        Trace::Memory => vitals.memory.fraction(),
        Trace::Gpu => vitals.main_gpu().map_or(0.0, |gpu| gpu.usage),
        Trace::Receive => vitals.traffic().receive,
        Trace::Send => vitals.traffic().send,
        Trace::Io => {
            let io = vitals.io();
            io.read + io.write
        }
    }
}

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;
use std::time::{Duration, Instant};

use rikka_renderer::{Node, Renderer, Scene};
use rikka_types::{Color, Rect};

use crate::allocations::{Phase, take_traces};
use crate::gpu_times::GpuTimes;
use crate::scenario::Scenario;

const FLUSH_FRAMES: usize = 4;

const SHOWN_PASSES: usize = 40;

pub(crate) const STEADY_AFTER: usize = 16;

const FRAME_PASS: &str = "rikka.frame";

#[derive(Clone, Copy, Debug)]
pub struct Pacing {
    pub period: Duration,
}

impl Pacing {
    pub fn hz(rate: u32) -> Pacing {
        Pacing {
            period: Duration::from_secs(1) / rate,
        }
    }
}

#[derive(Debug, Default)]
struct Frame {
    value: u64,
    cpu: Duration,
    gpu: f64,
    area: f32,
}

#[derive(Debug)]
pub struct Measured {
    scenario: Scenario,
    frames: Vec<Frame>,
    passes: BTreeMap<String, Vec<f64>>,
    steady_allocations: Option<u64>,
    steady_traces: Vec<String>,
}

impl Measured {
    pub fn frames(&self) -> usize {
        self.frames.len()
    }

    pub fn steady_allocations(&self) -> Option<u64> {
        self.steady_allocations
    }

    pub fn steady_traces(&self) -> &[String] {
        &self.steady_traces
    }

    fn gpu_percentile(&self, percent: f64) -> f64 {
        percentile(
            self.frames.iter().map(|frame| frame.gpu),
            percent,
            f64::total_cmp,
        )
    }

    fn cpu_percentile(&self, percent: f64) -> Duration {
        percentile(
            self.frames.iter().map(|frame| frame.cpu),
            percent,
            Duration::cmp,
        )
    }
}

fn percentile<T: Copy + Default>(
    values: impl Iterator<Item = T>,
    percent: f64,
    order: impl FnMut(&T, &T) -> Ordering,
) -> T {
    let mut values: Vec<T> = values.collect();
    values.sort_unstable_by(order);
    let at = ((values.len() as f64 - 1.0) * percent / 100.0).round() as usize;
    values.get(at).copied().unwrap_or_default()
}

impl fmt::Display for Measured {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let count = self.frames.len().max(1) as f64;
        writeln!(
            formatter,
            "{} — {} frames, {:.0} kpx painted per frame",
            self.scenario.name(),
            self.frames.len(),
            self.frames.iter().map(|frame| frame.area).sum::<f32>() / count as f32 / 1000.0,
        )?;
        let (worst_at, worst) = self
            .frames
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.gpu.total_cmp(&b.1.gpu))
            .map_or((0, 0.0), |(at, frame)| (at, frame.gpu));
        writeln!(
            formatter,
            "  gpu µs   p50 {:>8.1} p99 {:>8.1} max {:>8.1} mean {:>8.1}  worst #{worst_at}",
            self.gpu_percentile(50.0),
            self.gpu_percentile(99.0),
            worst,
            self.frames.iter().map(|frame| frame.gpu).sum::<f64>() / count,
        )?;
        let (cpu_worst_at, cpu_worst) = self
            .frames
            .iter()
            .enumerate()
            .max_by_key(|(_, frame)| frame.cpu)
            .map_or((0, Duration::ZERO), |(at, frame)| (at, frame.cpu));
        writeln!(
            formatter,
            "  cpu µs   p50 {:>8} p99 {:>8} max {:>8}  worst #{cpu_worst_at}",
            self.cpu_percentile(50.0).as_micros(),
            self.cpu_percentile(99.0).as_micros(),
            cpu_worst.as_micros(),
        )?;
        if let Some(allocated) = self.steady_allocations {
            writeln!(
                formatter,
                "  allocations per steady frame, at most {allocated}"
            )?;
        }
        let mut passes: Vec<(&String, f64, f64, f64)> = self
            .passes
            .iter()
            .map(|(name, samples)| {
                let mut sorted = samples.clone();
                sorted.sort_unstable_by(f64::total_cmp);
                let median = sorted.get(sorted.len() / 2).copied().unwrap_or_default();
                let most = sorted.last().copied().unwrap_or_default();
                (name, sorted.iter().sum::<f64>() / count, median, most)
            })
            .collect();
        passes.sort_unstable_by(|a, b| b.1.total_cmp(&a.1));
        for (name, mean, median, most) in passes.into_iter().take(SHOWN_PASSES) {
            writeln!(
                formatter,
                "    {name:<28} mean {mean:>8.1} p50 {median:>8.1} max {most:>8.1}"
            )?;
        }
        Ok(())
    }
}

pub fn measure(
    renderer: &mut Renderer,
    times: &GpuTimes,
    scenario: Scenario,
    frames: usize,
    pacing: Pacing,
) -> Measured {
    let mut running = scenario.open(renderer).expect("scenario should open");
    running.first(renderer).expect("first frame should draw");
    let mut measured = Measured {
        scenario,
        frames: Vec::with_capacity(frames),
        passes: BTreeMap::new(),
        steady_allocations: None,
        steady_traces: Vec::new(),
    };
    for at in 0..frames {
        let started = Instant::now();
        running.change(renderer, at).expect("frame should change");
        let changed = Instant::now();
        let phase = match at >= STEADY_AFTER {
            true => Phase::Steady,
            false => Phase::Warming,
        };
        let area = running
            .frame(renderer, at, phase)
            .expect("frame should draw");
        let cpu = changed.elapsed();
        if phase == Phase::Steady
            && let Some(allocated) = running.allocated()
        {
            measured.steady_allocations = Some(
                measured
                    .steady_allocations
                    .map_or(allocated, |most| most.max(allocated)),
            );
        }
        measured.frames.push(Frame {
            value: match area > 0.0 {
                true => renderer.submitted_frame(),
                false => 0,
            },
            cpu,
            gpu: 0.0,
            area,
        });
        while started.elapsed() < pacing.period {
            std::hint::spin_loop();
        }
    }
    measured.steady_traces = take_traces();
    flush(renderer);
    let drawn = measured
        .frames
        .iter()
        .map(|frame| frame.value)
        .filter(|value| *value > 0);
    let first = drawn.clone().min().unwrap_or(0);
    let last = drawn.max().unwrap_or(0);
    let mut per_frame: BTreeMap<u64, BTreeMap<String, f64>> = BTreeMap::new();
    for pass in times.take(first, last) {
        *per_frame
            .entry(pass.frame)
            .or_default()
            .entry(pass.name)
            .or_default() += pass.micros;
    }
    for frame in &mut measured.frames {
        let Some(passes) = per_frame.get(&frame.value) else {
            continue;
        };
        frame.gpu = passes.get(FRAME_PASS).copied().unwrap_or_default();
        for (name, micros) in passes {
            measured
                .passes
                .entry(name.clone())
                .or_default()
                .push(*micros);
        }
    }
    measured
}

fn flush(renderer: &mut Renderer) {
    let mut target = renderer.create_target(8, 8).expect("flush target");
    let mut scene = Scene::new();
    scene.insert(Node::rect(Rect::new(0.0, 0.0, 8.0, 8.0)).fill(Color::WHITE));
    for _ in 0..FLUSH_FRAMES {
        scene.damage_all();
        renderer
            .draw(&mut scene, &mut target)
            .expect("flush frame should draw");
    }
}

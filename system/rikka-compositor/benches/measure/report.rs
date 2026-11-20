use std::fmt;

use super::clocks::Sample;
use super::gpu_window::Presented;
use super::probe::{GpuFrame, Painted, Turn};
use super::timeline::Timeline;

const FIRST: usize = 10;

#[derive(Debug, Default)]
pub(crate) struct Measured {
    pub(crate) name: &'static str,
    pub(crate) turns: Vec<Turn>,
    pub(crate) painted: Vec<Painted>,
    pub(crate) gpu: Vec<GpuFrame>,
    pub(crate) clocks: Vec<Sample>,
    pub(crate) client: Vec<Presented>,
    pub(crate) screen: f64,
    pub(crate) imports: usize,
    pub(crate) busy: std::time::Duration,
    pub(crate) timelines: Vec<Timeline>,
}

impl Measured {
    pub(crate) fn painted(&self) -> usize {
        self.painted.len()
    }

    pub(crate) fn passes(&self) -> String {
        let mut spent: Vec<(&str, f64)> = Vec::new();
        for (name, millis) in self.gpu.iter().flat_map(|frame| &frame.passes) {
            match spent.iter_mut().find(|(seen, _)| *seen == name.as_str()) {
                Some((_, total)) => *total += millis,
                None => spent.push((name, *millis)),
            }
        }
        let frames = self.gpu.len().max(1) as f64;
        spent
            .iter()
            .map(|(name, total)| format!("{name}={:.3}", total / frames))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn percentile(values: &[f64], at: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let index = ((sorted.len() - 1) as f64 * at).round() as usize;
    sorted[index]
}

fn largest(values: &[f64]) -> f64 {
    values.iter().copied().fold(0.0, f64::max)
}

fn mean(values: &[f64]) -> f64 {
    match values.is_empty() {
        true => 0.0,
        false => values.iter().sum::<f64>() / values.len() as f64,
    }
}

pub(crate) fn header() -> String {
    format!(
        "{:<13} {:>6} {:>6} | {:>7} {:>7} {:>7} | {:>7} {:>7} {:>7} {:>7} {:>8} | {:>7} {:>6} | {:>9}",
        "scenario",
        "turns",
        "drawn",
        "cpu p50",
        "cpu p99",
        "cpu max",
        "gpu p50",
        "gpu p99",
        "gpu max",
        "blur50",
        "first10",
        "area%",
        "rects",
        "MHz gr/m",
    )
}

impl fmt::Display for Measured {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let drawn: Vec<&Turn> = self.turns.iter().filter(|turn| turn.rendered).collect();
        let cpu: Vec<f64> = drawn
            .iter()
            .map(|turn| turn.work.as_secs_f64() * 1000.0)
            .collect();
        let composed: Vec<f64> = drawn
            .iter()
            .map(|turn| turn.composed.as_secs_f64() * 1000.0)
            .collect();
        let gpu: Vec<f64> = self.gpu.iter().map(|frame| frame.total).collect();
        let blur: Vec<f64> = self.gpu.iter().map(|frame| frame.backdrop).collect();
        let first: Vec<f64> = gpu.iter().copied().take(FIRST).collect();
        let area: Vec<f64> = self
            .painted
            .iter()
            .map(|painted| painted.area / self.screen * 100.0)
            .collect();
        let rects: Vec<f64> = self
            .painted
            .iter()
            .map(|painted| painted.rects as f64)
            .collect();
        let graphics: Vec<f64> = self
            .clocks
            .iter()
            .map(|sample| f64::from(sample.graphics))
            .collect();
        let memory: Vec<f64> = self
            .clocks
            .iter()
            .map(|sample| f64::from(sample.memory))
            .collect();
        write!(
            formatter,
            "{:<13} {:>6} {:>6} | {:>7.3} {:>7.3} {:>7.3} | {:>7.3} {:>7.3} {:>7.3} {:>7.3} {:>8.3} | {:>7.1} {:>6.1} | {:>4.0}/{:<4.0}",
            self.name,
            self.turns.len(),
            self.painted.len(),
            percentile(&cpu, 0.5),
            percentile(&cpu, 0.99),
            largest(&cpu),
            percentile(&gpu, 0.5),
            percentile(&gpu, 0.99),
            largest(&gpu),
            percentile(&blur, 0.5),
            largest(&first),
            mean(&area),
            mean(&rects),
            percentile(&graphics, 0.5),
            percentile(&memory, 0.5),
        )?;
        let used: Vec<u32> = self.clocks.iter().map(|sample| sample.used).collect();
        let vram = format!(
            "{}/{}/{}",
            used.first().copied().unwrap_or_default(),
            used.iter().max().copied().unwrap_or_default(),
            used.last().copied().unwrap_or_default(),
        );
        let thread = self.busy.as_secs_f64() * 1000.0 / self.painted.len().max(1) as f64;
        write!(
            formatter,
            "\n{:<13} compose p50={:.3} p99={:.3} max={:.3} thread/frame={thread:.3} vram(MiB start/peak/end)={vram} dmabuf imports={} {}",
            "",
            percentile(&composed, 0.5),
            percentile(&composed, 0.99),
            largest(&composed),
            self.imports,
            self.passes()
        )?;
        if self.client.is_empty() {
            return Ok(());
        }
        let millis = |of: fn(&Presented) -> Option<std::time::Duration>| -> Vec<f64> {
            self.client
                .iter()
                .filter_map(of)
                .map(|spent| spent.as_secs_f64() * 1000.0)
                .collect()
        };
        let drawing = millis(|presented| Some(presented.drawing));
        let answered = millis(|presented| presented.answered);
        let resized = self
            .client
            .iter()
            .filter(|presented| presented.resized)
            .count();
        let configures: usize = self
            .client
            .iter()
            .map(|presented| presented.configures)
            .sum();
        write!(
            formatter,
            "\n{:<13} client: {} presents, {resized} new sizes, {configures} configures; draw+present p50={:.3} p99={:.3} max={:.3}; configure->present p50={:.3} p99={:.3} max={:.3}",
            "",
            self.client.len(),
            percentile(&drawing, 0.5),
            percentile(&drawing, 0.99),
            largest(&drawing),
            percentile(&answered, 0.5),
            percentile(&answered, 0.99),
            largest(&answered),
        )
    }
}

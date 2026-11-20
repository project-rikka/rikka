use std::time::Duration;

use crate::{reading, topology};

#[derive(Clone, Debug, Default)]
pub struct CpuSample {
    pub usage: f32,
    pub cores: Vec<f32>,
    pub ghz: f32,
    pub uptime: Duration,
    pub processes: usize,
    pub threads: usize,
    pub handles: u64,
}

#[derive(Debug, Default)]
pub struct Cpu {
    total: Vec<Times>,
}

#[derive(Clone, Copy, Debug, Default)]
struct Times {
    busy: u64,
    total: u64,
}

impl Times {
    fn parse(line: &str) -> Self {
        let fields: Vec<u64> = reading::numbers(line, 1).collect();
        let total: u64 = fields.iter().sum();
        let idle: u64 = fields.iter().skip(3).take(2).sum();
        Self {
            busy: total.saturating_sub(idle),
            total,
        }
    }

    fn usage_since(self, previous: Self) -> f32 {
        let total = self.total.saturating_sub(previous.total);
        if total == 0 {
            return 0.0;
        }
        (self.busy.saturating_sub(previous.busy) as f32 / total as f32).clamp(0.0, 1.0)
    }
}

impl Cpu {
    #[must_use]
    pub fn new() -> Self {
        let mut probe = Self::default();
        probe.sample();
        probe
    }

    pub fn sample(&mut self) -> CpuSample {
        let stat = reading::text("/proc/stat").unwrap_or_default();
        let times: Vec<Times> = stat
            .lines()
            .take_while(|line| line.starts_with("cpu"))
            .map(Times::parse)
            .collect();
        let usage: Vec<f32> = times
            .iter()
            .enumerate()
            .map(|(index, times)| {
                times.usage_since(self.total.get(index).copied().unwrap_or_default())
            })
            .collect();
        self.total = times;

        let (processes, threads) = counts();
        CpuSample {
            usage: usage.first().copied().unwrap_or_default(),
            cores: usage.into_iter().skip(1).collect(),
            ghz: ghz(),
            uptime: uptime(),
            processes,
            threads,
            handles: handles(),
        }
    }
}

fn ghz() -> f32 {
    let mut total = 0.0;
    let mut counted = 0;
    for cpu in topology::processors() {
        let path = format!("{}/{cpu}/cpufreq/scaling_cur_freq", topology::CPU_DIR);
        if let Some(khz) = reading::number::<f32>(path) {
            total += khz / 1_000_000.0;
            counted += 1;
        }
    }
    if counted > 0 {
        return total / counted as f32;
    }
    let cpuinfo = reading::text("/proc/cpuinfo").unwrap_or_default();
    let clocks: Vec<f32> = cpuinfo
        .lines()
        .filter_map(|line| line.split_once(':'))
        .filter(|(name, _)| name.trim() == "cpu MHz")
        .filter_map(|(_, value)| value.trim().parse::<f32>().ok())
        .collect();
    if clocks.is_empty() {
        return 0.0;
    }
    clocks.iter().sum::<f32>() / clocks.len() as f32 / 1000.0
}

fn uptime() -> Duration {
    reading::text("/proc/uptime")
        .and_then(|uptime| uptime.split_whitespace().next()?.parse().ok())
        .map(Duration::from_secs_f64)
        .unwrap_or_default()
}

fn counts() -> (usize, usize) {
    let processes = std::fs::read_dir("/proc").map_or(0, |entries| {
        entries
            .flatten()
            .filter(|entry| {
                entry
                    .file_name()
                    .as_encoded_bytes()
                    .iter()
                    .all(u8::is_ascii_digit)
            })
            .count()
    });
    let threads = reading::text("/proc/loadavg")
        .and_then(|loadavg| {
            let entities = loadavg.split_whitespace().nth(3)?;
            entities.split_once('/')?.1.parse().ok()
        })
        .unwrap_or(processes);
    (processes, threads)
}

fn handles() -> u64 {
    reading::text("/proc/sys/fs/file-nr")
        .and_then(|nr| nr.split_whitespace().next()?.parse().ok())
        .unwrap_or_default()
}

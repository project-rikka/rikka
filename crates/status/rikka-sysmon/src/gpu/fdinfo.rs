use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::{Duration, Instant};

const DEVICES: &str = "/dev/dri/";

const SWEPT: Duration = Duration::from_secs(4);

type Client = (String, u64);

#[derive(Debug, Default)]
pub(crate) struct Usage {
    pub(crate) adapters: HashMap<String, f32>,
    pub(crate) memory: HashMap<String, HashMap<String, u64>>,
    pub(crate) processes: HashMap<i32, f32>,
}

#[derive(Debug)]
pub(crate) struct Clients {
    previous: HashMap<Client, HashMap<String, u64>>,
    drawing: HashSet<i32>,
    read_at: Instant,
    swept_at: Instant,
}

impl Default for Clients {
    fn default() -> Self {
        Self {
            previous: HashMap::new(),
            drawing: HashSet::new(),
            read_at: Instant::now(),
            swept_at: Instant::now() - SWEPT,
        }
    }
}

impl Clients {
    pub(crate) fn sample(&mut self) -> Usage {
        let nanoseconds = self.read_at.elapsed().as_nanos() as f64;
        self.read_at = Instant::now();
        let mut usage = Usage::default();
        if nanoseconds <= 0.0 {
            return usage;
        }

        let sweeping = self.swept_at.elapsed() >= SWEPT;
        let readings = match sweeping {
            true => {
                self.swept_at = Instant::now();
                swept()
            }
            false => read_of(&self.drawing),
        };
        if sweeping {
            self.drawing = readings.iter().map(|(pid, _)| *pid).collect();
        }

        let mut current: HashMap<Client, HashMap<String, u64>> = HashMap::new();
        let mut engines: HashMap<String, HashMap<String, u64>> = HashMap::new();
        for (pid, reading) in readings {
            let client = (reading.slot.clone(), reading.client);
            if current.contains_key(&client) {
                continue;
            }
            let previous = self.previous.get(&client);
            let mut busiest = 0u64;
            for (engine, nanos) in &reading.engines {
                let before = previous
                    .and_then(|per_client| per_client.get(engine))
                    .copied()
                    .unwrap_or(*nanos);
                let delta = nanos.saturating_sub(before);
                busiest = busiest.max(delta);
                *engines
                    .entry(reading.slot.clone())
                    .or_default()
                    .entry(engine.clone())
                    .or_default() += delta;
            }
            let share = (busiest as f64 / nanoseconds) as f32;
            let per_client = usage.processes.entry(pid).or_default();
            *per_client = (*per_client + share).clamp(0.0, 1.0);

            let memory = usage.memory.entry(reading.slot.clone()).or_default();
            for (region, bytes) in &reading.memory {
                *memory.entry(region.clone()).or_default() += bytes;
            }
            current.insert(client, reading.engines);
        }
        self.previous = current;

        for (slot, classes) in engines {
            let busiest = classes.into_values().max().unwrap_or_default();
            usage
                .adapters
                .insert(slot, (busiest as f64 / nanoseconds).clamp(0.0, 1.0) as f32);
        }
        usage
    }
}

struct Reading {
    slot: String,
    client: u64,
    engines: HashMap<String, u64>,
    memory: HashMap<String, u64>,
}

fn swept() -> Vec<(i32, Reading)> {
    let Ok(processes) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for process in processes.flatten() {
        let Some(pid) = process
            .file_name()
            .into_string()
            .ok()
            .and_then(|name| name.parse::<i32>().ok())
        else {
            continue;
        };
        read_pid(pid, &process.path(), &mut found);
    }
    found
}

fn read_of(drawing: &HashSet<i32>) -> Vec<(i32, Reading)> {
    let mut found = Vec::new();
    for pid in drawing {
        read_pid(*pid, Path::new(&format!("/proc/{pid}")), &mut found);
    }
    found
}

fn read_pid(pid: i32, process: &Path, found: &mut Vec<(i32, Reading)>) {
    let Ok(descriptors) = std::fs::read_dir(process.join("fd")) else {
        return;
    };
    for descriptor in descriptors.flatten() {
        if !std::fs::read_link(descriptor.path()).is_ok_and(|target| target.starts_with(DEVICES)) {
            continue;
        }
        let path = process.join("fdinfo").join(descriptor.file_name());
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        if let Some(reading) = parse(&text) {
            found.push((pid, reading));
        }
    }
}

fn parse(text: &str) -> Option<Reading> {
    if !text.contains("drm-client-id") {
        return None;
    }
    let mut slot = None;
    let mut client = None;
    let mut engines = HashMap::new();
    let mut memory = HashMap::new();
    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match key {
            "drm-pdev" => slot = Some(value.to_owned()),
            "drm-client-id" => client = value.parse().ok(),
            _ => {
                if let Some(engine) = key.strip_prefix("drm-engine-") {
                    if let Some(nanos) = value.strip_suffix(" ns").and_then(|ns| ns.parse().ok()) {
                        engines.insert(engine.to_owned(), nanos);
                    }
                } else if let Some(region) = key.strip_prefix("drm-total-")
                    && let Some(bytes) = size(value)
                {
                    memory.insert(region.to_owned(), bytes);
                }
            }
        }
    }
    Some(Reading {
        slot: slot?,
        client: client?,
        engines,
        memory,
    })
}

fn size(value: &str) -> Option<u64> {
    let (number, unit) = value.split_once(' ')?;
    let number: u64 = number.parse().ok()?;
    Some(match unit {
        "KiB" => number * 1024,
        "MiB" => number * 1024 * 1024,
        "GiB" => number * 1024 * 1024 * 1024,
        _ => number,
    })
}

use std::collections::HashMap;
use std::time::Instant;

use crate::reading;

const SECTOR: u64 = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiskKind {
    Nvme,
    Ssd,
    Hdd,
}

#[derive(Clone, Debug)]
pub struct DiskSample {
    pub name: String,
    pub model: String,
    pub kind: DiskKind,
    pub capacity: u64,
    pub system: bool,
    pub active: f32,
    pub read: f32,
    pub write: f32,
    pub response_ms: f32,
    pub queue: u64,
}

#[derive(Clone, Copy, Debug, Default)]
struct Counters {
    reads: u64,
    read_sectors: u64,
    read_ms: u64,
    writes: u64,
    write_sectors: u64,
    write_ms: u64,
    in_flight: u64,
    busy_ms: u64,
}

impl Counters {
    fn parse(line: &str) -> Option<(String, Self)> {
        let name = line.split_whitespace().nth(2)?.to_owned();
        let fields: Vec<u64> = reading::numbers(line, 3).collect();
        let at = |index: usize| fields.get(index).copied().unwrap_or_default();
        (fields.len() >= 10).then(|| {
            (
                name,
                Self {
                    reads: at(0),
                    read_sectors: at(2),
                    read_ms: at(3),
                    writes: at(4),
                    write_sectors: at(6),
                    write_ms: at(7),
                    in_flight: at(8),
                    busy_ms: at(9),
                },
            )
        })
    }
}

#[derive(Clone, Debug)]
struct Identity {
    model: String,
    kind: DiskKind,
    capacity: u64,
}

impl Identity {
    fn read(name: &str) -> Self {
        Self {
            model: model(name),
            kind: kind(name),
            capacity: reading::number::<u64>(format!("/sys/block/{name}/size")).unwrap_or_default()
                * SECTOR,
        }
    }
}

#[derive(Debug)]
pub struct Disks {
    previous: HashMap<String, Counters>,
    identities: HashMap<String, Identity>,
    system: Option<String>,
    read_at: Instant,
}

impl Default for Disks {
    fn default() -> Self {
        Self::new()
    }
}

impl Disks {
    #[must_use]
    pub fn new() -> Self {
        let mut probe = Self {
            previous: HashMap::new(),
            identities: HashMap::new(),
            system: None,
            read_at: Instant::now(),
        };
        probe.sample();
        probe
    }

    pub fn sample(&mut self) -> Vec<DiskSample> {
        let seconds = self.read_at.elapsed().as_secs_f32();
        self.read_at = Instant::now();
        let diskstats = reading::text("/proc/diskstats").unwrap_or_default();

        let mut current = HashMap::new();
        for line in diskstats.lines() {
            let Some((name, counters)) = Counters::parse(line) else {
                continue;
            };
            if is_disk(&name) {
                current.insert(name, counters);
            }
        }
        self.identify(&current);

        let mut disks: Vec<DiskSample> = current
            .iter()
            .map(|(name, &counters)| {
                let previous = self.previous.get(name).copied().unwrap_or(counters);
                let identity = self
                    .identities
                    .get(name)
                    .expect("every disk should have been identified");
                DiskSample {
                    system: self.system.as_deref() == Some(name.as_str()),
                    model: identity.model.clone(),
                    kind: identity.kind,
                    capacity: identity.capacity,
                    active: (reading::rate(counters.busy_ms, previous.busy_ms, seconds) / 1000.0)
                        .clamp(0.0, 1.0),
                    read: reading::rate(counters.read_sectors, previous.read_sectors, seconds)
                        * SECTOR as f32,
                    write: reading::rate(counters.write_sectors, previous.write_sectors, seconds)
                        * SECTOR as f32,
                    response_ms: response(counters, previous),
                    queue: counters.in_flight,
                    name: name.clone(),
                }
            })
            .collect();
        self.previous = current;
        disks.sort_by(|left, right| left.name.cmp(&right.name));
        disks
    }

    fn identify(&mut self, current: &HashMap<String, Counters>) {
        let complete = current.len() == self.identities.len()
            && current
                .keys()
                .all(|name| self.identities.contains_key(name));
        if complete {
            return;
        }
        self.identities.retain(|name, _| current.contains_key(name));
        for name in current.keys() {
            if !self.identities.contains_key(name) {
                self.identities.insert(name.clone(), Identity::read(name));
            }
        }
        self.system = system_disk();
    }
}

fn response(counters: Counters, previous: Counters) -> f32 {
    let requests = counters
        .reads
        .saturating_sub(previous.reads)
        .saturating_add(counters.writes.saturating_sub(previous.writes));
    if requests == 0 {
        return 0.0;
    }
    let io_time = counters
        .read_ms
        .saturating_sub(previous.read_ms)
        .saturating_add(counters.write_ms.saturating_sub(previous.write_ms));
    io_time as f32 / requests as f32
}

fn is_disk(name: &str) -> bool {
    !name.starts_with("sr") && std::path::Path::new(&format!("/sys/block/{name}/device")).exists()
}

fn kind(name: &str) -> DiskKind {
    if name.starts_with("nvme") {
        return DiskKind::Nvme;
    }
    match reading::number::<u8>(format!("/sys/block/{name}/queue/rotational")) {
        Some(1) => DiskKind::Hdd,
        _ => DiskKind::Ssd,
    }
}

fn model(name: &str) -> String {
    reading::text(format!("/sys/block/{name}/device/model"))
        .or_else(|| reading::text(format!("/sys/block/{name}/device/modalias")))
        .map(|model| model.trim().to_owned())
        .filter(|model| !model.is_empty())
        .unwrap_or_else(|| name.to_owned())
}

fn system_disk() -> Option<String> {
    let mounts = reading::text("/proc/mounts")?;
    let root = mounts.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        let device = fields.next()?;
        (fields.next()? == "/").then(|| device.rsplit('/').next().unwrap_or(device).to_owned())
    })?;
    Some(holder(&root, 0))
}

fn holder(name: &str, depth: usize) -> String {
    if depth >= 8 {
        return name.to_owned();
    }
    if std::path::Path::new(&format!("/sys/class/block/{name}/partition")).exists() {
        let resolved = std::fs::canonicalize(format!("/sys/class/block/{name}"))
            .ok()
            .and_then(|path| Some(path.parent()?.file_name()?.to_str()?.to_owned()));
        if let Some(parent) = resolved {
            return holder(&parent, depth + 1);
        }
    }
    match reading::entries(format!("/sys/class/block/{name}/slaves")).first() {
        Some(slave) => holder(slave, depth + 1),
        None => name.to_owned(),
    }
}

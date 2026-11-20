use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::reading;

const POWERCAP: &str = "/sys/class/powercap";

#[derive(Debug)]
struct Zone {
    energy: PathBuf,
    range: u64,
    previous: Option<u64>,
}

#[derive(Debug)]
pub(crate) struct Rapl {
    zones: Vec<Zone>,
    read_at: Instant,
}

impl Rapl {
    pub(crate) fn new() -> Rapl {
        Rapl::with_root(POWERCAP)
    }

    fn with_root(root: impl AsRef<Path>) -> Rapl {
        let root = root.as_ref();
        let mut packages: Vec<String> = reading::entries(root)
            .into_iter()
            .filter(|zone| package(zone))
            .collect();
        packages.sort();
        let zones = packages
            .into_iter()
            .filter_map(|zone| {
                let dir = root.join(zone);
                let energy = dir.join("energy_uj");
                let previous = Some(reading::number::<u64>(&energy)?);
                Some(Zone {
                    range: reading::number(dir.join("max_energy_range_uj")).unwrap_or(u64::MAX),
                    energy,
                    previous,
                })
            })
            .collect();
        Rapl {
            zones,
            read_at: Instant::now(),
        }
    }

    pub(crate) fn watts(&mut self) -> Option<f32> {
        if self.zones.is_empty() {
            return None;
        }
        let seconds = self.read_at.elapsed().as_secs_f64();
        self.read_at = Instant::now();
        let mut joules = 0.0;
        let mut measured = false;
        for zone in &mut self.zones {
            let Some(now) = reading::number::<u64>(&zone.energy) else {
                continue;
            };
            if let Some(before) = zone.previous.replace(now) {
                let spent = match now >= before {
                    true => now - before,
                    false => zone.range.saturating_sub(before) + now,
                };
                joules += spent as f64 / 1e6;
                measured = true;
            }
        }
        (measured && seconds > 0.0).then(|| (joules / seconds) as f32)
    }
}

fn package(zone: &str) -> bool {
    zone.strip_prefix("intel-rapl:")
        .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|byte| byte.is_ascii_digit()))
}

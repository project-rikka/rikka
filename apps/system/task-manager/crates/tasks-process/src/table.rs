use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::cpus;
use crate::desktop::Applications;
use crate::group::{self, Group, Member};
use crate::process::{self, Process};
use crate::stat::{self, Identity, Stat};
use crate::users::Users;

const SLOW_REFRESH: Duration = Duration::from_secs(4);

#[derive(Debug)]
struct Previous {
    start_time: u64,
    ticks: u64,
    io: Option<u64>,
    memory: Option<u64>,
    identity: Identity,
    refreshed_at: Instant,
}

#[derive(Debug)]
pub struct Processes {
    applications: Applications,
    users: Users,
    previous: HashMap<i32, Previous>,
    read_at: Instant,
    clock_ticks: f32,
    online_cpus: f32,
}

impl Default for Processes {
    fn default() -> Self {
        Self::new()
    }
}

impl Processes {
    #[must_use]
    pub fn new() -> Self {
        let mut table = Self {
            applications: Applications::scan(),
            users: Users::default(),
            previous: HashMap::new(),
            read_at: Instant::now(),
            clock_ticks: rustix::param::clock_ticks_per_second() as f32,
            online_cpus: cpus::online() as f32,
        };
        table.sample(&HashMap::new());
        table
    }

    pub fn sample(&mut self, gpu: &HashMap<i32, f32>) -> Vec<Group> {
        let seconds = self.read_at.elapsed().as_secs_f32();
        self.read_at = Instant::now();

        let mut previous = std::mem::take(&mut self.previous);
        let mut members = Vec::new();
        for pid in pids() {
            let Some(stat) = Stat::read(pid) else {
                continue;
            };
            let known = previous
                .remove(&pid)
                .filter(|known| known.start_time == stat.start_time);
            let (last_ticks, last_io) = known
                .as_ref()
                .map_or((stat.ticks, None), |known| (known.ticks, known.io));
            let fresh = known.filter(|known| known.refreshed_at.elapsed() < SLOW_REFRESH);
            let (identity, memory, refreshed_at, io_readable) = match fresh {
                Some(known) => (
                    known.identity,
                    known.memory,
                    known.refreshed_at,
                    known.io.is_some(),
                ),
                None => {
                    let Some(identity) = Identity::read(pid) else {
                        continue;
                    };
                    let memory = identity.footprint(pid);
                    (identity, memory, Instant::now(), true)
                }
            };
            let io = io_readable.then(|| stat::io(pid)).flatten();
            let disk = match (io, last_io) {
                (Some(io), Some(last)) => rate(io, last, seconds),
                _ => 0.0,
            };

            let kernel = identity.is_kernel();
            let application = (!kernel)
                .then(|| self.applications.name(&identity.program))
                .flatten();
            let name = application.map_or_else(|| name(pid, &stat, &identity), str::to_owned);
            members.push(Member {
                system: kernel || identity.uid == 0,
                application: application.is_some(),
                process: Process {
                    pid,
                    start_time: stat.start_time,
                    parent: stat.parent,
                    search_text: process::search_text(&name, &identity.command),
                    name,
                    user: self.users.name(identity.uid),
                    state: stat.state,
                    cpu: self.share(stat.ticks, last_ticks, seconds),
                    memory,
                    disk,
                    gpu: gpu.get(&pid).copied().unwrap_or_default(),
                    threads: stat.threads,
                    command: identity.command.clone(),
                },
                program: identity.program.clone(),
            });
            self.previous.insert(
                pid,
                Previous {
                    start_time: stat.start_time,
                    ticks: stat.ticks,
                    io,
                    memory,
                    identity,
                    refreshed_at,
                },
            );
        }
        group::assemble(members)
    }

    fn share(&self, ticks: u64, previous: u64, seconds: f32) -> f32 {
        if seconds <= 0.0 {
            return 0.0;
        }
        let used = ticks.saturating_sub(previous) as f32 / self.clock_ticks;
        (used / seconds / self.online_cpus).clamp(0.0, 1.0)
    }
}

fn name(pid: i32, stat: &Stat, identity: &Identity) -> String {
    if !stat.comm.is_empty() {
        return stat.comm.clone();
    }
    if !identity.program.is_empty() {
        return identity.program.clone();
    }
    pid.to_string()
}

fn rate(current: u64, previous: u64, seconds: f32) -> f32 {
    if seconds <= 0.0 {
        return 0.0;
    }
    current.saturating_sub(previous) as f32 / seconds
}

fn pids() -> Vec<i32> {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok()?.parse().ok())
        .collect()
}

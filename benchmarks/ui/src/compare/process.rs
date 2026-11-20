use std::collections::HashMap;
use std::io;
use std::time::Duration;

use super::error::Error;

#[derive(Clone, Copy, Debug, Default)]
struct Thread {
    cpu: Duration,
    runs: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct ProcessSample {
    pid: u32,
    cpu: Duration,
    threads: HashMap<u32, Thread>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Usage {
    pub(crate) cpu: Duration,
    pub(crate) main_thread: Duration,
    pub(crate) runs: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize)]
pub struct Memory {
    pub rss: u64,
    pub pss: u64,
    pub private: u64,
}

impl ProcessSample {
    pub(crate) fn take(pid: u32) -> Result<ProcessSample, Error> {
        let cpu = process_cpu(pid).map_err(|source| Error::Process { pid, source })?;
        let tasks = std::fs::read_dir(format!("/proc/{pid}/task"))
            .map_err(|source| Error::Process { pid, source })?;
        let mut threads = HashMap::new();
        for entry in tasks.flatten() {
            let Some(tid) = entry.file_name().to_str().and_then(|tid| tid.parse().ok()) else {
                continue;
            };
            let Ok(schedstat) = std::fs::read_to_string(entry.path().join("schedstat")) else {
                continue;
            };
            let mut fields = schedstat.split_whitespace().map(str::parse::<u64>);
            let (Some(Ok(cpu)), Some(Ok(_)), Some(Ok(runs))) =
                (fields.next(), fields.next(), fields.next())
            else {
                continue;
            };
            threads.insert(
                tid,
                Thread {
                    cpu: Duration::from_nanos(cpu),
                    runs,
                },
            );
        }
        Ok(ProcessSample { pid, cpu, threads })
    }

    #[must_use]
    pub(crate) fn since(&self, before: &ProcessSample) -> Usage {
        let mut usage = Usage {
            cpu: self.cpu.saturating_sub(before.cpu),
            ..Usage::default()
        };
        for (tid, thread) in &self.threads {
            let earlier = before.threads.get(tid).copied().unwrap_or_default();
            usage.runs += thread.runs.saturating_sub(earlier.runs);
            if *tid == self.pid {
                usage.main_thread = thread.cpu.saturating_sub(earlier.cpu);
            }
        }
        usage
    }
}

impl std::ops::Add for Usage {
    type Output = Usage;

    fn add(self, other: Usage) -> Usage {
        Usage {
            cpu: self.cpu + other.cpu,
            main_thread: self.main_thread + other.main_thread,
            runs: self.runs + other.runs,
        }
    }
}

fn process_cpu(pid: u32) -> io::Result<Duration> {
    let mut clock: libc::clockid_t = 0;
    let found = unsafe { libc::clock_getcpuclockid(pid as libc::pid_t, &mut clock) };
    if found != 0 {
        return Err(io::Error::from_raw_os_error(found));
    }
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    if unsafe { libc::clock_gettime(clock, &mut time) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(Duration::new(time.tv_sec as u64, time.tv_nsec as u32))
}

pub(crate) fn memory(pid: u32) -> Result<Memory, Error> {
    let rollup = std::fs::read_to_string(format!("/proc/{pid}/smaps_rollup"))
        .map_err(|source| Error::Process { pid, source })?;
    let mut memory = Memory::default();
    for line in rollup.lines() {
        let mut fields = line.split_whitespace();
        let (Some(name), Some(Ok(kib))) = (fields.next(), fields.next().map(str::parse::<u64>))
        else {
            continue;
        };
        let bytes = kib * 1024;
        match name {
            "Rss:" => memory.rss = bytes,
            "Pss:" => memory.pss = bytes,
            "Private_Clean:" | "Private_Dirty:" => memory.private += bytes,
            _ => {}
        }
    }
    Ok(memory)
}

#[must_use]
pub(crate) fn children_named(parent: u32, name: &str) -> Vec<u32> {
    std::fs::read_dir("/proc")
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.file_name().to_str()?.parse::<u32>().ok())
        .filter(|pid| {
            let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
                return false;
            };
            let Some((comm, rest)) = stat
                .split_once('(')
                .and_then(|(_, rest)| rest.rsplit_once(')'))
            else {
                return false;
            };
            let ppid = rest
                .split_whitespace()
                .nth(1)
                .and_then(|ppid| ppid.parse::<u32>().ok());
            comm == name && ppid == Some(parent)
        })
        .collect()
}

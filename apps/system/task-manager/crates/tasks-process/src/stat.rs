use std::os::unix::fs::MetadataExt;

use crate::process::State;

#[derive(Debug)]
pub(crate) struct Stat {
    pub(crate) parent: i32,
    pub(crate) comm: String,
    pub(crate) state: State,
    pub(crate) threads: u32,
    pub(crate) ticks: u64,
    pub(crate) start_time: u64,
}

impl Stat {
    pub(crate) fn read(pid: i32) -> Option<Self> {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        let (head, tail) = stat.rsplit_once(") ")?;
        let (_, comm) = head.split_once('(')?;
        let fields: Vec<&str> = tail.split_whitespace().collect();
        let field = |index: usize| fields.get(index).copied();
        let number = |index: usize| field(index)?.parse::<u64>().ok();
        Some(Self {
            parent: field(1)?.parse().ok()?,
            comm: comm.to_owned(),
            state: State::parse(field(0)?),
            threads: field(17)?.parse().ok()?,
            ticks: number(11)? + number(12)?,
            start_time: number(19)?,
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Identity {
    pub(crate) command: String,
    pub(crate) program: String,
    pub(crate) uid: u32,
}

impl Identity {
    pub(crate) fn read(pid: i32) -> Option<Self> {
        let directory = format!("/proc/{pid}");
        let uid = std::fs::metadata(&directory).ok()?.uid();
        let command = command(&directory);
        Some(Self {
            program: command
                .split_whitespace()
                .next()
                .and_then(|argv0| argv0.rsplit('/').next())
                .unwrap_or_default()
                .to_owned(),
            command,
            uid,
        })
    }

    pub(crate) fn is_kernel(&self) -> bool {
        self.command.is_empty()
    }

    pub(crate) fn footprint(&self, pid: i32) -> Option<u64> {
        if self.is_kernel() {
            return Some(0);
        }
        let rollup = std::fs::read_to_string(format!("/proc/{pid}/smaps_rollup")).ok()?;
        footprint_of(&rollup)
    }
}

pub(crate) fn io(pid: i32) -> Option<u64> {
    let io = std::fs::read_to_string(format!("/proc/{pid}/io")).ok()?;
    let bytes = |key| field(&io, key).unwrap_or_default();
    Some(bytes("read_bytes:") + bytes("write_bytes:"))
}

fn footprint_of(rollup: &str) -> Option<u64> {
    let dirty = field(rollup, "Pss_Dirty:").or_else(|| field(rollup, "Private_Dirty:"))?;
    let swapped = field(rollup, "SwapPss:").unwrap_or_default();
    Some((dirty + swapped) * 1024)
}

fn field(text: &str, key: &str) -> Option<u64> {
    text.lines()
        .find_map(|line| line.strip_prefix(key))?
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}

fn command(directory: &str) -> String {
    let Ok(raw) = std::fs::read(format!("{directory}/cmdline")) else {
        return String::new();
    };
    let text = String::from_utf8_lossy(&raw);
    text.trim_end_matches('\0').replace('\0', " ")
}

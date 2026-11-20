use std::collections::{HashMap, VecDeque};

use crate::ansi;
use crate::message::{Field, Message};
use crate::tally::{Count, Tally};

const PREFIX: &str = "@nix ";

const OUTPUT_TAIL: usize = 4;

const ERROR: u8 = 0;

const INFO: u8 = 3;

const COPY_PATH: u64 = 100;
const FILE_TRANSFER: u64 = 101;
const REALISE: u64 = 102;
const COPY_PATHS: u64 = 103;
const BUILDS: u64 = 104;
const BUILD: u64 = 105;
const POST_BUILD_HOOK: u64 = 110;

const BUILD_LOG_LINE: u64 = 101;
const PROGRESS: u64 = 105;
const SET_EXPECTED: u64 = 106;
const POST_BUILD_LOG_LINE: u64 = 107;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Realisation {
    Pending,
    Running,
    Finished,
}

#[derive(Debug, Default)]
pub struct Decoder {
    activities: HashMap<u64, Activity>,
    totals: HashMap<u64, Totals>,
    realising: usize,
    realised: bool,
    error: Option<String>,
    output: VecDeque<String>,
}

#[derive(Debug)]
struct Activity {
    kind: u64,
    name: Option<String>,
    count: Count,
    expected: HashMap<u64, u64>,
}

#[derive(Clone, Copy, Debug, Default)]
struct Totals {
    done: u64,
    failed: u64,
    expected: u64,
}

impl Decoder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn decode(&mut self, line: &str) -> Vec<String> {
        let message = line
            .strip_prefix(PREFIX)
            .and_then(|json| serde_json::from_str::<Message>(json).ok());
        match message {
            Some(message) => self.apply(message),
            None => vec![self.output(line)],
        }
    }

    #[must_use]
    pub fn tally(&self) -> Tally {
        Tally {
            builds: self.count(BUILDS),
            paths: self.count(COPY_PATHS),
            copied: self.count(COPY_PATH),
            downloaded: self.count(FILE_TRANSFER),
        }
    }

    #[must_use]
    pub fn realisation(&self) -> Realisation {
        match (self.realising, self.realised) {
            (0, false) => Realisation::Pending,
            (0, true) => Realisation::Finished,
            _ => Realisation::Running,
        }
    }

    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn output_tail(&self) -> impl Iterator<Item = &str> {
        self.output.iter().map(String::as_str)
    }

    fn output(&mut self, line: &str) -> String {
        let shown = plain(line);
        if self.output.len() == OUTPUT_TAIL {
            self.output.pop_front();
        }
        self.output.push_back(shown.clone());
        shown
    }

    fn apply(&mut self, message: Message) -> Vec<String> {
        match message {
            Message::Start {
                id,
                level,
                text,
                kind,
                fields,
            } => {
                self.start(id, kind, &fields);
                match level <= INFO && !text.is_empty() {
                    true => vec![plain(&text)],
                    false => Vec::new(),
                }
            }
            Message::Stop { id } => {
                self.stop(id);
                Vec::new()
            }
            Message::Result { id, kind, fields } => self.result(id, kind, &fields),
            Message::Msg {
                level,
                msg,
                raw_msg,
            } => self.message(level, &msg, raw_msg.as_deref()),
            Message::Other => Vec::new(),
        }
    }

    fn start(&mut self, id: u64, kind: u64, fields: &[Field]) {
        let name = matches!(kind, BUILD | POST_BUILD_HOOK)
            .then(|| fields.first().and_then(Field::text).map(derivation_name))
            .flatten();
        if kind == REALISE {
            self.realising += 1;
        }
        self.activities.insert(
            id,
            Activity {
                kind,
                name,
                count: Count::default(),
                expected: HashMap::new(),
            },
        );
    }

    fn stop(&mut self, id: u64) {
        let Some(activity) = self.activities.remove(&id) else {
            return;
        };
        let totals = self.totals.entry(activity.kind).or_default();
        totals.done = totals.done.wrapping_add(activity.count.done);
        totals.failed = totals.failed.wrapping_add(activity.count.failed);
        for (kind, expected) in activity.expected {
            let totals = self.totals.entry(kind).or_default();
            totals.expected = totals.expected.wrapping_sub(expected);
        }
        if activity.kind == REALISE {
            self.realising -= 1;
            self.realised = true;
        }
    }

    fn result(&mut self, id: u64, kind: u64, fields: &[Field]) -> Vec<String> {
        let number = |at: usize| fields.get(at).and_then(Field::number).unwrap_or(0);
        match kind {
            BUILD_LOG_LINE | POST_BUILD_LOG_LINE => {
                let Some(line) = fields.first().and_then(Field::text) else {
                    return Vec::new();
                };
                let line = plain(line);
                let name = self
                    .activities
                    .get(&id)
                    .and_then(|activity| activity.name.as_deref());
                vec![match name {
                    Some(name) => format!("{name}> {line}"),
                    None => line,
                }]
            }
            PROGRESS => {
                if let Some(activity) = self.activities.get_mut(&id) {
                    activity.count = Count {
                        done: number(0),
                        expected: number(1),
                        running: number(2),
                        failed: number(3),
                    };
                }
                Vec::new()
            }
            SET_EXPECTED => {
                let (counted, expected) = (number(0), number(1));
                if let Some(activity) = self.activities.get_mut(&id) {
                    let previous = activity.expected.insert(counted, expected).unwrap_or(0);
                    let totals = self.totals.entry(counted).or_default();
                    totals.expected = totals
                        .expected
                        .wrapping_sub(previous)
                        .wrapping_add(expected);
                }
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn message(&mut self, level: u8, text: &str, bare: Option<&str>) -> Vec<String> {
        if level > INFO {
            return Vec::new();
        }
        let shown: Vec<String> = text.lines().map(plain).collect();
        if level == ERROR {
            self.error = Some(match bare {
                Some(bare) => bare.lines().map(plain).collect::<Vec<_>>().join("\n"),
                None => shown.join("\n"),
            });
        }
        shown
    }

    fn count(&self, kind: u64) -> Count {
        let totals = self.totals.get(&kind).copied().unwrap_or_default();
        let mut count = Count {
            done: totals.done,
            expected: totals.done,
            running: 0,
            failed: totals.failed,
        };
        for activity in self
            .activities
            .values()
            .filter(|activity| activity.kind == kind)
        {
            count.done = count.done.wrapping_add(activity.count.done);
            count.expected = count.expected.wrapping_add(activity.count.expected);
            count.running = count.running.wrapping_add(activity.count.running);
            count.failed = count.failed.wrapping_add(activity.count.failed);
        }
        count.expected = count.expected.max(totals.expected);
        count
    }
}

fn plain(line: &str) -> String {
    let line = line.trim_end_matches('\r');
    let shown = line.rsplit('\r').next().unwrap_or(line);
    ansi::readable(shown).into_owned()
}

fn derivation_name(path: &str) -> String {
    let file = path.rsplit('/').next().unwrap_or(path);
    let named = file.split_once('-').map_or(file, |(_, name)| name);
    named.strip_suffix(".drv").unwrap_or(named).to_owned()
}

use rikka_sync::Mutex;
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};

use rikka_worker::{Stop, Worker};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rikka_nvml::Nvml;

use super::interval::share;

const POLL: Duration = Duration::from_millis(80);

const HELD_MICROS: u64 = 2_500_000;

const FORGOTTEN_MICROS: u64 = 30_000_000;

#[derive(Debug, Default)]
struct User {
    latest: u64,
    busy: u64,
    samples: u32,
    shown: f32,
}

type Tally = HashMap<u32, HashMap<u32, User>>;

pub(super) struct Recorder {
    tally: Arc<Mutex<Tally>>,
    worker: Option<Worker<Halt>>,
}

struct Halt(Sender<()>);

impl Stop for Halt {
    fn stop(&self) {
        let _ = self.0.send(());
    }
}

impl Recorder {
    pub(super) fn start(nvml: Arc<Nvml>) -> Recorder {
        let tally = Arc::new(Mutex::new(Tally::new()));
        let (stop, stopped) = mpsc::channel();
        let shared = Arc::clone(&tally);
        let worker = Worker::spawn("rikka-gpu-users", Halt(stop), move || {
            Ok::<_, Infallible>(move || {
                let mut since = now_micros();
                loop {
                    let looked = now_micros();
                    record(&nvml, &shared, since, looked);
                    since = looked;
                    match stopped.recv_timeout(POLL) {
                        Err(RecvTimeoutError::Timeout) => {}
                        Ok(()) | Err(RecvTimeoutError::Disconnected) => return,
                    }
                }
            })
        })
        .inspect_err(|error| tracing::debug!("per-process GPU accounting unavailable: {error}"))
        .ok();
        Recorder { tally, worker }
    }

    pub(super) fn take(&self, index: u32) -> HashMap<u32, f32> {
        let since = now_micros().saturating_sub(HELD_MICROS);
        let mut tally = self.tally.lock();
        let Some(users) = tally.get_mut(&index) else {
            return HashMap::new();
        };
        users
            .iter_mut()
            .filter_map(|(pid, user)| {
                if user.samples > 0 {
                    user.shown = share(user.busy as f32 / user.samples as f32);
                    user.busy = 0;
                    user.samples = 0;
                } else if user.latest <= since {
                    return None;
                }
                Some((*pid, user.shown))
            })
            .collect()
    }
}

impl std::fmt::Debug for Recorder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Recorder")
            .field("running", &self.worker.is_some())
            .finish_non_exhaustive()
    }
}

fn record(nvml: &Nvml, tally: &Mutex<Tally>, since: u64, looked: u64) {
    let count = nvml.device_count().unwrap_or(0);
    for index in 0..count {
        let Ok(samples) = nvml
            .device_by_index(index)
            .and_then(|device| device.process_utilization_stats(None))
        else {
            continue;
        };
        let mut tally = tally.lock();
        let users = tally.entry(index).or_default();
        for sample in samples {
            let user = users.entry(sample.pid).or_insert_with(|| User {
                latest: since,
                ..User::default()
            });
            if sample.timestamp <= user.latest {
                continue;
            }
            user.latest = sample.timestamp;
            user.busy += u64::from(sample.sm_util);
            user.samples += 1;
        }
        let forgotten = looked.saturating_sub(FORGOTTEN_MICROS);
        users.retain(|_, user| user.latest > forgotten || user.samples > 0);
    }
}

fn now_micros() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_micros() as u64)
}

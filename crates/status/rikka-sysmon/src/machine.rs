use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::{Duration, Instant};

use futures_util::Stream;
use rikka_worker::{Observable, Stop, Worker};

use crate::gpu::Gpu;
use crate::load;
use crate::powercap::Rapl;
use crate::probe::Probe;
use crate::sensors::Hwmon;
use crate::storage::{self, Volume};
use crate::topology::Topology;
use crate::vitals::Vitals;

const NAME: &str = "rikka-sysmon";
const CADENCE: Duration = Duration::from_secs(1);
const BASELINE_INTERVAL: Duration = Duration::from_millis(250);
const REMEASURED: Duration = Duration::from_secs(30);

#[derive(Clone, Debug)]
pub struct Machine(Arc<Shared>);

struct Shared {
    vitals: Observable<Arc<Vitals>>,
    sampler: Option<Worker<Control>>,
}

impl Machine {
    #[must_use]
    pub fn start() -> Machine {
        Machine::with_interval(CADENCE)
    }

    #[must_use]
    pub fn with_interval(cadence: Duration) -> Machine {
        let vitals = Observable::new(Arc::new(Vitals::default()));
        let (control, signals) = channel();
        let publishing = vitals.clone();
        let sampler = Worker::spawn(NAME, Control(control), move || {
            Ok::<_, std::convert::Infallible>(move || sample(&publishing, &signals, cadence))
        })
        .inspect_err(|error| tracing::warn!("failed to start system monitor: {error}"))
        .ok();
        Machine(Arc::new(Shared { vitals, sampler }))
    }

    #[must_use]
    pub fn state(&self) -> Arc<Vitals> {
        self.0.vitals.get()
    }

    pub fn changes(&self) -> impl Stream<Item = Arc<Vitals>> + Send + 'static {
        let changes = self.0.vitals.changes();
        if let Some(sampler) = &self.0.sampler {
            sampler.handle().wake();
        }
        changes
    }
}

impl std::fmt::Debug for Shared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Machine")
            .field("sampling", &self.sampler.is_some())
            .field("watchers", &self.vitals.receiver_count())
            .finish()
    }
}

enum Signal {
    Wake,
    Stop,
}

struct Control(Sender<Signal>);

impl Control {
    fn wake(&self) {
        let _ = self.0.send(Signal::Wake);
    }
}

impl Stop for Control {
    fn stop(&self) {
        let _ = self.0.send(Signal::Stop);
    }
}

struct Sampler {
    probe: Probe,
    topology: Arc<Topology>,
    gpu: Gpu,
    hwmon: Hwmon,
    rapl: Rapl,
    volumes: Vec<Volume>,
    measured_at: Option<Instant>,
}

impl Sampler {
    fn new() -> Sampler {
        let probe = Probe::new();
        Sampler {
            topology: Arc::new(probe.topology().clone()),
            probe,
            gpu: Gpu::new(),
            hwmon: Hwmon::new(),
            rapl: Rapl::new(),
            volumes: Vec::new(),
            measured_at: None,
        }
    }

    fn read(&mut self) -> Vitals {
        let machine = self.probe.sample();
        if self.measured_at.is_none_or(|at| at.elapsed() >= REMEASURED) {
            self.volumes = storage::volumes();
            self.measured_at = Some(Instant::now());
        }
        Vitals {
            topology: Arc::clone(&self.topology),
            cpu: machine.cpu,
            cpu_power: self.rapl.watts(),
            load: load::read(),
            memory: machine.memory,
            gpus: self.gpu.adapters(),
            disks: machine.disks,
            volumes: self.volumes.clone(),
            networks: machine.networks,
            sensors: self.hwmon.read(),
        }
    }
}

fn sample(vitals: &Observable<Arc<Vitals>>, signals: &Receiver<Signal>, cadence: Duration) {
    let mut sampler: Option<Sampler> = None;
    let mut watched = false;
    loop {
        if vitals.receiver_count() == 0 {
            watched = false;
            match signals.recv() {
                Ok(Signal::Wake) => continue,
                Ok(Signal::Stop) | Err(_) => return,
            }
        }
        let reading = sampler.get_or_insert_with(Sampler::new);
        if !watched {
            watched = true;
            reading.read();
            if wait_or_stop(signals, BASELINE_INTERVAL) {
                return;
            }
        }
        vitals.set(Arc::new(reading.read()));
        if wait_or_stop(signals, cadence) {
            return;
        }
    }
}

fn wait_or_stop(signals: &Receiver<Signal>, period: Duration) -> bool {
    let until = Instant::now() + period;
    loop {
        let left = until.saturating_duration_since(Instant::now());
        match signals.recv_timeout(left) {
            Ok(Signal::Wake) => {}
            Ok(Signal::Stop) | Err(RecvTimeoutError::Disconnected) => return true,
            Err(RecvTimeoutError::Timeout) => return false,
        }
    }
}

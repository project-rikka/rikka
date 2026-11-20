use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::panic::{self, AssertUnwindSafe};
use std::sync::Once;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const CASE_LIMIT: Duration = Duration::from_secs(20);
const SLOW_CASE: Duration = Duration::from_secs(1);
const SHOWN_LABELS: usize = 3;

thread_local! {
    static PANIC: RefCell<Option<String>> = const { RefCell::new(None) };
    static IN_CASE: Cell<bool> = const { Cell::new(false) };
}

fn install_hook() {
    static HOOK: Once = Once::new();
    HOOK.call_once(|| {
        let previous = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            if !IN_CASE.with(Cell::get) {
                previous(info);
                return;
            }
            let message = match info.payload().downcast_ref::<&str>() {
                Some(text) => (*text).to_owned(),
                None => info
                    .payload()
                    .downcast_ref::<String>()
                    .cloned()
                    .unwrap_or_default(),
            };
            let location = info
                .location()
                .map(|location| format!("{}:{}", location.file(), location.line()))
                .unwrap_or_default();
            let text = format!("panic at {location}: {message}");
            eprintln!("case {text}");
            PANIC.with(|slot| *slot.borrow_mut() = Some(text));
        }));
    });
}

#[derive(Debug)]
struct Failure {
    labels: Vec<String>,
    count: usize,
}

#[derive(Debug)]
pub struct Sweep {
    name: String,
    cases: usize,
    failures: BTreeMap<String, Failure>,
    notes: BTreeMap<String, usize>,
    tolerated: Vec<String>,
    watchdog: Option<(Sender<String>, JoinHandle<()>)>,
}

impl Sweep {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Sweep {
        install_hook();
        let (sender, receiver) = mpsc::channel::<String>();
        let handle = thread::Builder::new()
            .name("rikka-test-meshes-watchdog".to_owned())
            .spawn(move || {
                let mut current = String::new();
                loop {
                    match receiver.recv_timeout(CASE_LIMIT) {
                        Ok(label) => current = label,
                        Err(RecvTimeoutError::Disconnected) => return,
                        Err(RecvTimeoutError::Timeout) => {
                            eprintln!("case exceeded {CASE_LIMIT:?}: {current}");
                            std::process::abort();
                        }
                    }
                }
            });
        let handle = match handle {
            Ok(handle) => handle,
            Err(error) => panic!("failed to start watchdog: {error}"),
        };
        Sweep {
            name: name.into(),
            cases: 0,
            notes: BTreeMap::new(),
            tolerated: Vec::new(),
            failures: BTreeMap::new(),
            watchdog: Some((sender, handle)),
        }
    }

    pub fn run(&mut self, label: &str, case: impl FnOnce() -> Result<(), String>) -> bool {
        self.cases += 1;
        if let Some((sender, _)) = &self.watchdog
            && sender.send(label.to_owned()).is_err()
        {
            panic!("watchdog stopped");
        }
        PANIC.with(|slot| *slot.borrow_mut() = None);
        IN_CASE.with(|flag| flag.set(true));
        let started = Instant::now();
        let outcome = panic::catch_unwind(AssertUnwindSafe(case));
        IN_CASE.with(|flag| flag.set(false));
        if started.elapsed() > SLOW_CASE {
            eprintln!("slow case ({:?}): {label}", started.elapsed());
        }
        let failure = match outcome {
            Ok(Ok(())) => None,
            Ok(Err(error)) => Some(error),
            Err(_) => Some(
                PANIC
                    .with(|slot| slot.borrow_mut().take())
                    .unwrap_or_else(|| "panic".to_owned()),
            ),
        };
        let Some(message) = failure else {
            return true;
        };
        if self
            .tolerated
            .iter()
            .any(|prefix| message.starts_with(prefix.as_str()))
        {
            self.note(format!("known issue: {message}"));
            return false;
        }
        let entry = self.failures.entry(message).or_insert(Failure {
            labels: Vec::new(),
            count: 0,
        });
        entry.count += 1;
        if entry.labels.len() < SHOWN_LABELS {
            entry.labels.push(label.to_owned());
        }
        false
    }

    pub fn tolerate(&mut self, prefix: impl Into<String>) {
        self.tolerated.push(prefix.into());
    }

    pub fn note(&mut self, text: String) {
        *self.notes.entry(text).or_insert(0) += 1;
    }

    #[must_use]
    pub fn cases(&self) -> usize {
        self.cases
    }

    pub fn finish(mut self) {
        if let Some((sender, handle)) = self.watchdog.take() {
            drop(sender);
            if handle.join().is_err() {
                panic!("watchdog panicked");
            }
        }
        eprintln!("{}: {} cases", self.name, self.cases);
        for (note, count) in &self.notes {
            eprintln!("  {note} ({count} cases)");
        }
        if self.failures.is_empty() {
            return;
        }
        match self.render() {
            Ok(report) => {
                eprintln!("{report}");
                panic!("{report}")
            }
            Err(error) => panic!("failed to render sweep report: {error}"),
        }
    }

    fn render(&self) -> Result<String, std::fmt::Error> {
        let mut report = String::new();
        let total: usize = self.failures.values().map(|failure| failure.count).sum();
        writeln!(
            report,
            "{}: {total} of {} cases failed",
            self.name, self.cases
        )?;
        for (message, failure) in &self.failures {
            writeln!(report, "- {message} ({} cases)", failure.count)?;
            for label in &failure.labels {
                writeln!(report, "    {label}")?;
            }
        }
        Ok(report)
    }
}

use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

use rikka_sync::{Mutex, MutexGuard};

use crate::error::Result;
use crate::exit;
use crate::ffi;
use crate::program::{Program, Source};
use crate::worker::{Generated, MODULE_EXTENSION, Worker};

const WORKERS: usize = 8;

const RENEW_THREAD: &str = "rikka-slang-renew";

static HELPERS: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Default)]
#[must_use]
pub struct CompilerBuilder {
    modules: HashMap<String, Box<str>>,
    optimization: Optimization,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Optimization {
    #[default]
    High,
    None,
}

impl Optimization {
    pub(crate) fn level(self) -> i32 {
        match self {
            Self::High => ffi::OPTIMIZATION_HIGH,
            Self::None => ffi::OPTIMIZATION_NONE,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Settings {
    pub(crate) library: HashMap<String, Box<str>>,
    pub(crate) optimization: Optimization,
}

impl CompilerBuilder {
    pub fn module(mut self, name: &str, source: impl Into<Box<str>>) -> Self {
        let path = name.replace('.', "/") + MODULE_EXTENSION;
        self.modules.insert(path, source.into());
        self
    }

    pub fn modules<'a>(self, modules: impl IntoIterator<Item = &'a (&'a str, &'a str)>) -> Self {
        modules.into_iter().fold(self, |builder, (name, source)| {
            builder.module(name, *source)
        })
    }

    pub fn optimization(mut self, optimization: Optimization) -> Self {
        self.optimization = optimization;
        self
    }

    pub fn build(self) -> Result<Compiler> {
        let settings = Arc::new(Settings {
            library: self.modules,
            optimization: self.optimization,
        });
        let busy = exit::busy()?;
        let first = Worker::open(&settings)?;
        drop(busy);
        let workers = std::iter::once(Some(first))
            .chain(std::iter::repeat_with(|| None).take(WORKERS - 1))
            .map(Mutex::new)
            .collect();
        Ok(Compiler {
            inner: Arc::new(Inner {
                settings,
                workers,
                turn: AtomicUsize::new(0),
            }),
        })
    }
}

#[derive(Debug)]
pub struct Compiler {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    settings: Arc<Settings>,
    workers: Box<[Mutex<Option<Worker>>]>,
    turn: AtomicUsize,
}

impl Compiler {
    pub fn builder() -> CompilerBuilder {
        CompilerBuilder::default()
    }

    pub fn new() -> Result<Self> {
        Self::builder().build()
    }

    pub fn compile(&self, source: &Source<'_>) -> Result<Program> {
        let busy = exit::busy()?;
        let mut primary = self.inner.primary();
        let wanted = source.entries.len().saturating_sub(1).min(WORKERS - 1);
        let helpers = Helpers::claim(wanted);
        let next = AtomicUsize::new(0);
        let (first, others) = std::thread::scope(|scope| {
            let running: Vec<_> = (0..helpers.count)
                .map(|_| {
                    let next = &next;
                    scope.spawn(move || self.inner.help(source, next))
                })
                .collect();
            let first = self.inner.run(&mut primary, |worker| {
                worker.generate(&self.inner.settings, source, &next, true)
            });
            let others: Vec<Result<Generated>> = running
                .into_iter()
                .map(|helper| {
                    helper
                        .join()
                        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
                })
                .collect();
            (first, others)
        });
        drop(primary);
        drop(helpers);
        self.renew_due(source);
        drop(busy);
        let first = first?;
        let mut entry_points = first.entry_points;
        for generated in others {
            entry_points.extend(generated?.entry_points);
        }
        entry_points.sort_by_key(|(index, _)| *index);
        let reflection = first
            .reflection
            .expect("the primary worker reflects the program");
        Ok(Program::new(
            entry_points.into_iter().map(|(_, entry)| entry).collect(),
            reflection.parameters,
            reflection.types,
            reflection.conformances,
        ))
    }

    pub fn reflect(&self, source: &Source<'_>) -> Result<Program> {
        let busy = exit::busy()?;
        let mut primary = self.inner.primary();
        let reflection = self.inner.run(&mut primary, |worker| {
            worker.reflect(&self.inner.settings, source)
        });
        drop(primary);
        self.renew_due(source);
        drop(busy);
        let reflection = reflection?;
        Ok(Program::new(
            Vec::new(),
            reflection.parameters,
            reflection.types,
            reflection.conformances,
        ))
    }

    fn renew_due(&self, source: &Source<'_>) {
        for (index, slot) in self.inner.workers.iter().enumerate() {
            let due = slot
                .try_lock()
                .is_some_and(|worker| worker.as_ref().is_some_and(Worker::is_due));
            if !due {
                continue;
            }
            let inner = Arc::clone(&self.inner);
            let (name, text) = (source.name.to_owned(), source.text.to_owned());
            let spawned = std::thread::Builder::new()
                .name(RENEW_THREAD.to_owned())
                .spawn(move || inner.renew(index, &Source::new(&name, &text)));
            if spawned.is_err() {
                self.inner.renew(index, source);
            }
        }
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        let workers = std::mem::take(&mut self.workers);
        match exit::busy() {
            Ok(_busy) => drop(workers),
            Err(_) => std::mem::forget(workers),
        }
    }
}

impl Inner {
    fn renew(&self, index: usize, source: &Source<'_>) {
        let Ok(_busy) = exit::busy() else {
            return;
        };
        let mut slot = self.workers[index].lock();
        let renewed = match slot.as_mut() {
            Some(worker) if worker.is_due() => worker.renew(&self.settings, source),
            _ => return,
        };
        if renewed.is_err() {
            *slot = None;
        }
    }

    fn help(&self, source: &Source<'_>, next: &AtomicUsize) -> Result<Generated> {
        match self.free() {
            Some(mut worker) => self.run(&mut worker, |worker| {
                worker.generate(&self.settings, source, next, false)
            }),
            None => Ok(Generated::default()),
        }
    }

    fn primary(&self) -> MutexGuard<'_, Option<Worker>> {
        self.free().unwrap_or_else(|| {
            let turn = self.turn.fetch_add(1, Ordering::Relaxed);
            self.workers[turn % self.workers.len()].lock()
        })
    }

    fn free(&self) -> Option<MutexGuard<'_, Option<Worker>>> {
        let mut empty = None;
        for slot in &self.workers {
            if let Some(worker) = slot.try_lock() {
                if worker.is_some() {
                    return Some(worker);
                }
                empty.get_or_insert(worker);
            }
        }
        empty
    }

    fn run<T>(
        &self,
        worker: &mut Option<Worker>,
        run: impl FnOnce(&mut Worker) -> Result<T>,
    ) -> Result<T> {
        match worker {
            Some(worker) => run(worker),
            empty => run(empty.insert(Worker::open(&self.settings)?)),
        }
    }
}

struct Helpers {
    count: usize,
}

impl Helpers {
    fn claim(wanted: usize) -> Self {
        let active = HELPERS.fetch_add(wanted, Ordering::AcqRel);
        let count = wanted.min(helper_limit().saturating_sub(active));
        HELPERS.fetch_sub(wanted - count, Ordering::AcqRel);
        Self { count }
    }
}

impl Drop for Helpers {
    fn drop(&mut self) {
        HELPERS.fetch_sub(self.count, Ordering::AcqRel);
    }
}

fn helper_limit() -> usize {
    static LIMIT: OnceLock<usize> = OnceLock::new();
    *LIMIT.get_or_init(|| {
        std::thread::available_parallelism()
            .map_or(1, NonZeroUsize::get)
            .saturating_sub(1)
    })
}

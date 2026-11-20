use std::sync::Arc;

use crate::error::Error;

#[derive(Clone, Debug)]
pub enum TemplateReadiness {
    Pending,
    Drawable,
    Built,
    Failed(Arc<Error>),
}

pub(crate) type Built = Result<(), Arc<Error>>;

type DrawableWaiter = Box<dyn FnOnce() + Send>;

type BuiltWaiter = Box<dyn FnOnce(Built) + Send>;

type StepWaiter = Box<dyn FnOnce() + Send>;

#[derive(Default)]
pub(crate) struct Progress {
    pub ready: u64,
    pub drawable: u64,
    pub failed: Option<(u64, Arc<Error>)>,
    pub optimizing: u64,
    drawable_waiters: Vec<(u64, DrawableWaiter)>,
    built_waiters: Vec<(u64, BuiltWaiter)>,
    step_waiters: Vec<StepWaiter>,
}

#[must_use]
pub(crate) struct Due {
    drawable: Vec<DrawableWaiter>,
    built: Vec<(BuiltWaiter, Built)>,
    stepped: Vec<StepWaiter>,
}

impl Due {
    pub(crate) fn notify(self) {
        self.drawable.into_iter().for_each(|waiter| waiter());
        self.built
            .into_iter()
            .for_each(|(waiter, result)| waiter(result));
        self.stepped.into_iter().for_each(|waiter| waiter());
    }
}

impl Progress {
    pub(crate) fn failure(&self, revision: u64) -> Option<&Arc<Error>> {
        self.failed
            .as_ref()
            .filter(|(failed, _)| *failed == revision)
            .map(|(_, error)| error)
    }

    pub(crate) fn wait_drawable(&mut self, revision: u64, waiter: DrawableWaiter) {
        self.drawable_waiters.push((revision, waiter));
    }

    pub(crate) fn wait_built(&mut self, revision: u64, waiter: BuiltWaiter) {
        self.built_waiters.push((revision, waiter));
    }

    pub(crate) fn wait_step(&mut self, waiter: StepWaiter) {
        self.step_waiters.push(waiter);
    }

    pub(crate) fn step(&mut self) -> Due {
        Due {
            drawable: Vec::new(),
            built: Vec::new(),
            stepped: std::mem::take(&mut self.step_waiters),
        }
    }

    pub(crate) fn settle(&mut self, built: Option<(u64, Built)>) -> Due {
        let drawable = self.drawable;
        let (due, waiting) = std::mem::take(&mut self.drawable_waiters)
            .into_iter()
            .partition(|(wanted, _)| *wanted <= drawable);
        self.drawable_waiters = waiting;
        let mut settled = Vec::new();
        if let Some((revision, result)) = built {
            let (due, waiting) = std::mem::take(&mut self.built_waiters)
                .into_iter()
                .partition(|(wanted, _)| *wanted <= revision);
            self.built_waiters = waiting;
            settled = due
                .into_iter()
                .map(|(_, waiter): (u64, BuiltWaiter)| (waiter, result.clone()))
                .collect();
        }
        Due {
            drawable: due.into_iter().map(|(_, waiter)| waiter).collect(),
            built: settled,
            stepped: std::mem::take(&mut self.step_waiters),
        }
    }
}

impl std::fmt::Debug for Progress {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Progress")
            .field("ready", &self.ready)
            .field("drawable", &self.drawable)
            .field("failed", &self.failed)
            .field("optimizing", &self.optimizing)
            .field("drawable_waiters", &self.drawable_waiters.len())
            .field("built_waiters", &self.built_waiters.len())
            .field("step_waiters", &self.step_waiters.len())
            .finish()
    }
}

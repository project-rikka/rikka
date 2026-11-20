use std::cell::{Cell, RefCell};
use std::rc::Rc;

use files_vfs::{Conflict, Operation, Outcome, Snapshot};
use files_wallpaper::Library;
use rikka_app::prelude::*;
use rikka_file_operations::Resolution;
use rikka_widgets::Flex;

use crate::window::Asking;
use crate::window::undo::Journal;
use crate::wording;

const UPDATES: &str = "files-job-updates";

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Showing {
    pub(crate) measured: bool,
    pub(crate) question: Option<Conflict>,
}

impl Showing {
    fn from_snapshot(snapshot: &Snapshot) -> Self {
        Self {
            measured: snapshot.progress.fraction().is_some(),
            question: snapshot.question.clone(),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) enum After {
    #[default]
    Nothing,
    Retry(Asking),
    Journal(Journal),
    ShowWallpaper(Library),
}

impl After {
    fn outlives_replacement(&self) -> bool {
        matches!(self, After::Journal(_) | After::ShowWallpaper(_))
    }
}

#[derive(Debug)]
pub(crate) struct Finished {
    pub(crate) outcome: Outcome,
    pub(crate) after: After,
}

#[derive(State)]
pub(crate) struct Jobs {
    showing: Tracked<Option<Showing>>,
    #[untracked]
    running: Option<Operation>,
    #[untracked]
    after: After,
    #[untracked]
    filled: Rc<Cell<f32>>,
    #[untracked]
    filling: Watchers,
    #[untracked]
    counted: Rc<RefCell<String>>,
    #[untracked]
    counting: Watchers,
    #[untracked]
    updated: Handler<(Snapshot,)>,
    #[untracked]
    finished: Handler<(Finished,)>,
}

impl Jobs {
    pub(crate) fn new<M>(finished: impl IntoHandler<(Finished,), (), M>) -> Self {
        let finished = finished.into_handler();
        let relayed = finished.clone();
        Self {
            showing: Tracked::new(None),
            running: None,
            after: After::default(),
            filled: Rc::default(),
            filling: Watchers::new(),
            counted: Rc::default(),
            counting: Watchers::new(),
            updated: Handler::relaying(move |reach, (snapshot,)| {
                let done = reach.get::<Jobs>()?.apply(snapshot)?;
                relayed.call(reach, (done,))
            }),
            finished,
        }
    }

    pub(crate) fn run(&mut self, job: Operation, after: After) {
        self.replace();
        let snapshot = job.snapshot();
        *self.counted.borrow_mut() = wording::job_status(job.kind(), &snapshot.progress);
        self.counting.rebuild();
        self.filled.set(0.0);
        self.filling.repaint();
        self.showing.set(Some(Showing::from_snapshot(&snapshot)));
        stream_keyed(UPDATES, job.updates(), self.updated.clone());
        self.running = Some(job);
        self.after = after;
    }

    fn replace(&mut self) {
        let Some(previous) = self.running.take() else {
            return;
        };
        previous.cancel();
        let after = std::mem::take(&mut self.after);
        if !after.outlives_replacement() {
            return;
        }
        let finished = self.finished.clone();
        let after = Cell::new(Some(after));
        spawn_blocking(
            move || previous.wait(),
            Handler::relaying(move |reach, (outcome,)| {
                let after = after.take()?;
                finished.call(reach, (Finished { outcome, after },))
            }),
        );
    }

    pub(crate) fn showing(&self) -> Option<&Showing> {
        self.showing.as_ref()
    }

    pub(crate) fn resolve(&mut self, resolution: Resolution, remember: bool) {
        if let Some(job) = &self.running {
            job.resolve(resolution, remember);
        }
    }

    pub(crate) fn cancel(&mut self) {
        if let Some(job) = &self.running {
            job.cancel();
        }
    }

    fn apply(&mut self, snapshot: Snapshot) -> Option<Finished> {
        let job = self.running.as_ref()?;
        let Some(outcome) = snapshot.outcome.clone() else {
            let filled = snapshot.progress.fraction().unwrap_or_default();
            if self.filled.replace(filled) != filled {
                self.filling.repaint();
            }
            let status = wording::job_status(job.kind(), &snapshot.progress);
            if *self.counted.borrow() != status {
                *self.counted.borrow_mut() = status;
                self.counting.rebuild();
            }
            self.showing.set(Some(Showing::from_snapshot(&snapshot)));
            return None;
        };
        self.showing.set(None);
        self.running = None;
        Some(Finished {
            outcome,
            after: std::mem::take(&mut self.after),
        })
    }
}

impl std::fmt::Debug for Jobs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Jobs")
            .field("showing", &self.showing)
            .field("running", &self.running.is_some())
            .finish_non_exhaustive()
    }
}

pub(crate) fn line(jobs: &Jobs) -> Flex {
    let filled = Rc::clone(&jobs.filled);
    let counted = Rc::clone(&jobs.counted);
    let measured = jobs.showing().is_some_and(|showing| showing.measured);
    row! {
        live(&jobs.counting, move || text(counted.borrow().clone()).caption());
        match measured {
            true => live_progress(&jobs.filling, move || filled.get()).width(160.0),
            false => spinner().extent(Length::px(14.0)),
        }
        button(text(tr!("files.job.cancel")).caption())
            .ghost()
            .small()
            .on_press(Jobs::cancel);
    }
}

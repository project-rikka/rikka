use std::cell::{Cell, RefCell};
use std::rc::Rc;

use ade_files::Job;
use rikka_app::prelude::*;
use rikka_file_operations::{Conflict, Entry, Item, Resolution, Snapshot};
use rikka_widgets::{column, row};

use crate::labels;
use crate::project::act::act;
use crate::project::state::Project;

const GLYPH: f32 = 14.0;
const BAR: f32 = 72.0;

pub(crate) struct Running {
    id: u64,
    job: Job,
    question: Option<Conflict<Entry>>,
    measured: bool,
    status: Rc<RefCell<String>>,
    counting: Watchers,
    filled: Rc<Cell<f32>>,
    filling: Watchers,
}

impl Running {
    pub(crate) fn new(id: u64, job: Job) -> Running {
        let snapshot = job.snapshot();
        Running {
            id,
            status: Rc::new(RefCell::new(labels::job_status(
                job.kind(),
                &snapshot.progress,
            ))),
            filled: Rc::new(Cell::new(snapshot.progress.fraction().unwrap_or_default())),
            measured: snapshot.progress.fraction().is_some(),
            question: snapshot.question,
            counting: Watchers::new(),
            filling: Watchers::new(),
            job,
        }
    }

    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    pub(crate) fn job(&self) -> &Job {
        &self.job
    }

    pub(crate) fn advance(&mut self, snapshot: Snapshot<Entry>) -> bool {
        let status = labels::job_status(self.job.kind(), &snapshot.progress);
        if *self.status.borrow() != status {
            *self.status.borrow_mut() = status;
            self.counting.rebuild();
        }
        let fraction = snapshot.progress.fraction();
        let filled = fraction.unwrap_or_default();
        if self.filled.replace(filled) != filled {
            self.filling.repaint();
        }
        let measured = fraction.is_some();
        let reshaped = measured != self.measured || snapshot.question != self.question;
        self.measured = measured;
        self.question = snapshot.question;
        reshaped
    }
}

impl std::fmt::Debug for Running {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Running")
            .field("id", &self.id)
            .field("kind", &self.job.kind())
            .field("question", &self.question)
            .finish_non_exhaustive()
    }
}

pub(crate) fn view(running: &Running) -> impl View {
    let id = running.id;
    let status = Rc::clone(&running.status);
    let filled = Rc::clone(&running.filled);
    column! {
        row! {
            live(&running.counting, move || {
                text(status.borrow().clone()).caption().muted().no_wrap()
            });
            spacer().grow(1.0);
            match running.measured {
                true => live_progress(&running.filling, move || filled.get()).width(BAR),
                false => spinner().extent(Length::px(GLYPH)),
            }
            button(text(tr!("ade.files.job.stop")).caption())
                .ghost()
                .small()
                .on_press(act(move |project| {
                    project.explore(|explorer, _| explorer.cancel_job(id));
                }));
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
        if let Some(question) = &running.question {
            text(tr!("ade.files.conflict.exists", name = question.source.name()))
                .caption()
                .medium();
            row! {
                answer(id, tr!("ade.files.conflict.replace"), Resolution::Replace, false);
                answer(id, tr!("ade.files.conflict.keep"), Resolution::Keep, false).secondary();
            }
            .gap(Space::TIGHT);
            row! {
                answer(id, tr!("ade.files.conflict.skip"), Resolution::Skip, false).ghost();
                answer(id, tr!("ade.files.conflict.skip-all"), Resolution::Skip, true).ghost();
            }
            .gap(Space::TIGHT);
        }
    }
    .gap(Space::TIGHT)
    .padding(Inset::xy(6.0, 4.0))
    .fill_width()
}

fn answer(id: u64, label: Label, resolution: Resolution, remember: bool) -> Button {
    button(text(label).caption())
        .small()
        .on_press(act(move |project: &mut Project| {
            project.explore(|explorer, _| explorer.resolve_job(id, resolution, remember));
        }))
}

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use launcher_game::{Level, LogLine};
use rikka_app::prelude::*;
use rikka_sync::Mutex;
use rikka_ui::{Element, Revision};

use crate::launcher::Launcher;

const KEPT_LINES: usize = 20_000;
const LINE: f32 = 20.0;

pub(crate) type Lines = Rc<RefCell<VecDeque<LogLine>>>;

#[derive(State, Default)]
pub(crate) struct Logs {
    #[untracked]
    buffers: HashMap<String, Lines>,
    revision: Revision,
}

impl std::fmt::Debug for Logs {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Logs")
            .field("instances", &self.buffers.len())
            .finish()
    }
}

#[derive(Clone, Default)]
pub(crate) struct Pending {
    lines: Arc<Mutex<Vec<LogLine>>>,
    scheduled: Arc<AtomicBool>,
}

impl Pending {
    pub(crate) fn push(&self, id: &str, line: LogLine, sender: &Sender<Launcher>) {
        self.lines.lock().push(line);
        if self.scheduled.swap(true, Ordering::AcqRel) {
            return;
        }
        let pending = self.clone();
        let id = id.to_owned();
        if sender
            .send_to(move |logs: &mut Logs| logs.flush(&id, &pending))
            .is_err()
        {
            tracing::debug!("game output after the window closed");
        }
    }
}

impl Logs {
    pub(crate) fn start(&mut self, id: &str) {
        self.buffers.insert(id.to_owned(), Rc::default());
        self.revision.changed();
    }

    fn flush(&mut self, id: &str, pending: &Pending) {
        pending.scheduled.store(false, Ordering::Release);
        let drained = std::mem::take(&mut *pending.lines.lock());
        let buffer = self.buffers.entry(id.to_owned()).or_default();
        let mut lines = buffer.borrow_mut();
        lines.extend(drained);
        let excess = lines.len().saturating_sub(KEPT_LINES);
        lines.drain(..excess);
        drop(lines);
        self.revision.changed();
    }

    pub(crate) fn lines(&self, id: &str) -> Option<Lines> {
        self.revision.read();
        self.buffers.get(id).cloned()
    }
}

pub(crate) fn level_color(level: Level) -> Color {
    match level {
        Level::Error | Level::Fatal => palette().danger,
        Level::Warn => palette().warning,
        Level::Debug | Level::Trace => palette().text_muted,
        Level::Info => palette().text,
    }
}

pub(crate) fn log_list(lines: Lines) -> Element {
    let count = lines.borrow().len();
    lazy_column(count, move |slot| {
        let lines = lines.borrow();
        let Some(line) = lines.get(slot.index) else {
            return Element::empty();
        };
        text(line.text.clone())
            .caption()
            .family(Family::Monospace)
            .color(level_color(line.level))
            .no_wrap()
            .any()
    })
    .estimate(LINE)
    .padding(Inset::all(Space::NORMAL))
    .fill()
    .any()
}

pub(crate) fn live(logs: &Logs, id: &Arc<str>) -> Element {
    match logs.lines(id) {
        Some(lines) => log_list(lines),
        None => status_page(MaterialSymbol::TERMINAL, tr!("launcher.logs.no-run"))
            .fill()
            .any(),
    }
}

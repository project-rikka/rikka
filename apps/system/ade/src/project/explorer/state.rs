use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use ade_files::{Job, Kind};
use ade_tree::{Row, Selection};
use rikka_app::prelude::*;
use rikka_file_operations::{Entry, Resolution, Snapshot};

use crate::project::explorer::jobs::Running;

#[derive(Debug)]
pub(crate) enum Naming {
    Rename {
        path: PathBuf,
        document: TextDocument,
    },
    Create {
        folder: PathBuf,
        kind: Kind,
        document: TextDocument,
    },
}

impl Naming {
    pub(crate) fn document(&self) -> &TextDocument {
        match self {
            Naming::Rename { document, .. } | Naming::Create { document, .. } => document,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Summoned {
    pub(crate) at: Point,
    pub(crate) on_row: bool,
    pub(crate) on_file: bool,
    pub(crate) pasteable: bool,
    pub(crate) key: u64,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Pressed {
    pub(crate) index: usize,
    pub(crate) at: Point,
}

#[derive(Debug, Default)]
pub(crate) struct Gesture {
    pub(crate) pressed: Option<Pressed>,
    pub(crate) aim: Option<PathBuf>,
}

#[derive(Debug, Default)]
pub(crate) struct Explorer {
    selection: Selection,
    naming: Option<Naming>,
    menu: Option<Summoned>,
    menus: u64,
    error_message: Option<Arc<str>>,
    erasing: Option<Vec<PathBuf>>,
    aim: Option<PathBuf>,
    revealing: Option<PathBuf>,
    committing: bool,
    gesture: Rc<RefCell<Gesture>>,
    jobs: Vec<Running>,
    started: u64,
}

impl Explorer {
    pub(crate) fn selection(&self) -> &Selection {
        &self.selection
    }

    pub(crate) fn naming(&self) -> Option<&Naming> {
        self.naming.as_ref()
    }

    pub(crate) fn menu(&self) -> Option<Summoned> {
        self.menu
    }

    pub(crate) fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    pub(crate) fn erasing(&self) -> Option<&[PathBuf]> {
        self.erasing.as_deref()
    }

    pub(crate) fn aim(&self) -> Option<&Path> {
        self.aim.as_deref()
    }

    pub(crate) fn revealing(&self) -> Option<&Path> {
        self.revealing.as_deref()
    }

    pub(crate) fn jobs(&self) -> &[Running] {
        &self.jobs
    }

    pub(crate) fn track(&mut self, job: Job) -> u64 {
        self.started += 1;
        self.jobs.push(Running::new(self.started, job));
        self.started
    }

    pub(crate) fn advance(&mut self, id: u64, snapshot: Snapshot<Entry>) -> bool {
        self.jobs
            .iter_mut()
            .find(|running| running.id() == id)
            .is_some_and(|running| running.advance(snapshot))
    }

    pub(crate) fn finish(&mut self, id: u64) {
        self.jobs.retain(|running| running.id() != id);
    }

    pub(crate) fn resolve_job(&mut self, id: u64, resolution: Resolution, remember: bool) {
        if let Some(running) = self.running(id) {
            running.job().resolve(resolution, remember);
        }
        skip_view();
    }

    pub(crate) fn cancel_job(&mut self, id: u64) {
        if let Some(running) = self.running(id) {
            running.job().cancel();
        }
        skip_view();
    }

    fn running(&self, id: u64) -> Option<&Running> {
        self.jobs.iter().find(|running| running.id() == id)
    }

    pub(crate) fn gesture(&self) -> Rc<RefCell<Gesture>> {
        Rc::clone(&self.gesture)
    }

    pub(crate) fn chosen(&mut self) -> &mut Selection {
        self.revealing = None;
        &mut self.selection
    }

    pub(crate) fn choose(&mut self, path: &Path) {
        self.selection.only(path);
        self.revealing = Some(path.to_owned());
    }

    pub(crate) fn summon(&mut self, at: Point, row: Option<&Row>, pasteable: bool) {
        self.menus += 1;
        self.menu = Some(Summoned {
            at,
            on_row: row.is_some(),
            on_file: row.is_some_and(|row| !row.is_folder()),
            pasteable,
            key: self.menus,
        });
    }

    pub(crate) fn dismiss_quietly(&mut self) {
        self.menu = None;
    }

    pub(crate) fn commit(&mut self) -> bool {
        !std::mem::replace(&mut self.committing, true)
    }

    pub(crate) fn dismiss(&mut self) {
        if self.menu.take().is_none() {
            skip_view();
        }
    }

    pub(crate) fn name(&mut self, naming: Naming) {
        self.menu = None;
        self.error_message = None;
        self.naming = Some(naming);
    }

    pub(crate) fn take_naming(&mut self) -> Option<Naming> {
        self.committing = false;
        self.error_message = None;
        self.naming.take()
    }

    pub(crate) fn report_error(&mut self, message: Arc<str>) {
        self.committing = false;
        self.error_message = Some(message);
    }

    pub(crate) fn forget_trouble(&mut self) {
        if self.error_message.take().is_none() {
            skip_view();
        }
    }

    pub(crate) fn ask_erasing(&mut self, paths: Vec<PathBuf>) {
        self.menu = None;
        self.erasing = (!paths.is_empty()).then_some(paths);
    }

    pub(crate) fn take_erasing(&mut self) -> Option<Vec<PathBuf>> {
        self.erasing.take()
    }

    pub(crate) fn aim_at(&mut self, aim: Option<PathBuf>) {
        if self.aim == aim {
            skip_view();
            return;
        }
        self.aim = aim;
    }

    pub(crate) fn keep(&mut self, exists: impl Fn(&Path) -> bool) {
        self.selection.keep(exists);
    }

    pub(crate) fn moved(&mut self, from: &Path, to: &Path) {
        self.selection.moved(from, to);
    }

    pub(crate) fn focused_row<'a>(&self, rows: &'a [Row]) -> Option<&'a Row> {
        let focus = self.selection.focus()?;
        rows.iter().find(|row| row.path() == focus)
    }
}

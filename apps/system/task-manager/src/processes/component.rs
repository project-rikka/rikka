use std::collections::HashSet;
use std::rc::Rc;

use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::processes::column::{Column, Sorting};
use crate::processes::context::context_area;
use crate::processes::grid::{Grid, Sizing};
use crate::processes::mode::Mode;
use crate::processes::row::{Builder, Scale};
use crate::processes::row_order::RowOrder;
use crate::processes::table::{RowTable, Snapshot};
use crate::sampler::Reading;
use tasks_process::{Error, Group, Process};

const SEARCH: f32 = 300.0;
const SEARCH_FLOOR: f32 = 120.0;
const MENU: f32 = 220.0;
const MARK: &str = "tasks-processes-mark";

#[derive(State)]
pub(crate) struct Processes {
    mode: Tracked<Mode>,
    sizings: Tracked<Vec<Sizing>>,
    #[untracked]
    reading: Rc<Reading>,
    #[untracked]
    table: RowTable,
    row_order: Tracked<RowOrder>,
    #[untracked]
    expanded: HashSet<i32>,
    selected: Tracked<Option<i32>>,
    focus: Tracked<Option<usize>>,
    sorting: Tracked<Sorting>,
    #[untracked]
    search: TextDocument,
    search_focused: Tracked<bool>,
    #[untracked]
    searched: u64,
    #[untracked]
    menu_target: Option<i32>,
    menu: Tracked<Option<(i32, Point)>>,
    error: Tracked<Option<Label>>,
    #[untracked]
    stale: bool,
}

impl Processes {
    pub(crate) fn new(mode: Mode) -> Self {
        let table = RowTable::new();
        Self {
            sizings: Tracked::new(mode.columns().into_iter().map(Sizing::new).collect()),
            mode: Tracked::new(mode),
            reading: Rc::new(Reading::default()),
            row_order: Tracked::new(RowOrder::new(table.peek(), false)),
            table,
            expanded: HashSet::new(),
            selected: Tracked::new(None),
            focus: Tracked::new(None),
            sorting: Tracked::new(Sorting::default()),
            search: TextDocument::new(""),
            search_focused: Tracked::new(false),
            searched: 0,
            menu_target: None,
            menu: Tracked::new(None),
            error: Tracked::new(None),
            stale: true,
        }
    }

    pub(crate) fn set_reading(&mut self, reading: Rc<Reading>) {
        self.reading = reading;
        self.stale = true;
    }

    pub(crate) fn rebuild_if_stale(&mut self) {
        if self.stale {
            self.rebuild(false);
        }
    }

    pub(crate) fn error(&self) -> Option<&Label> {
        self.error.as_ref()
    }

    pub(crate) fn select(&mut self, pid: i32) {
        self.selected.set(Some(pid));
        self.refocus();
        self.error.set(None);
    }

    pub(crate) fn toggle(&mut self, pid: i32) {
        if !self.expanded.remove(&pid) {
            self.expanded.insert(pid);
        }
        self.rebuild(true);
    }

    pub(crate) fn sort(&mut self, column: Column) {
        let sorting = self.sorting.toggled(column);
        self.sorting.set(sorting);
        self.rebuild(false);
    }

    pub(crate) fn resize(&mut self, position: usize, width: f32) {
        if self
            .sizings
            .get(position)
            .is_some_and(|sizing| sizing.width != width)
            && let Some(sizing) = self.sizings.get_mut(position)
        {
            sizing.width = width;
        }
    }

    pub(crate) fn target_menu(&mut self, pid: i32) {
        self.selected.set(Some(pid));
        self.refocus();
        self.menu_target = Some(pid);
    }

    fn open_menu(&mut self, at: Point) {
        if let Some(pid) = self.menu_target.take() {
            self.menu.set(Some((pid, at)));
        }
    }

    fn dismiss(&mut self) {
        self.menu.set(None);
    }

    pub(crate) fn focus_search(&mut self) {
        self.search.select_all();
        self.search_focused.set(true);
    }

    fn search_blurred(&mut self) {
        self.search_focused.set(false);
    }

    fn search(&mut self) {
        let revision = self.search.revision();
        if self.searched != revision {
            self.searched = revision;
            self.rebuild(false);
        }
    }

    pub(crate) fn end_task(&mut self, signal: fn(&Process) -> tasks_process::Result<()>) {
        self.menu.set(None);
        let Some(pid) = *self.selected else {
            return;
        };
        let snapshot = self.table.peek();
        let result = match snapshot
            .reading
            .groups
            .iter()
            .flat_map(Group::processes)
            .find(|process| process.pid == pid)
        {
            Some(process) => signal(process),
            None => Err(Error::NoSuchProcess(pid)),
        };
        self.error.set(match result {
            Ok(()) => None,
            Err(Error::NoSuchProcess(_)) => Some(tr!("tasks.end-error.no-such-process")),
            Err(Error::PermissionDenied(_)) => Some(tr!("tasks.end-error.permission-denied")),
            Err(Error::Signal { .. }) => Some(tr!("tasks.end-error.failed")),
        });
    }

    fn rebuild(&mut self, glides: bool) {
        self.stale = false;
        let scale = Scale::new(&self.reading.groups, self.reading.machine.memory.total);
        let builder = Builder::new(*self.sorting, &self.search.text(), &self.expanded);
        let rows = match *self.mode {
            Mode::Grouped => builder.grouped(&self.reading.groups),
            Mode::Flat => builder.flat(&self.reading.groups),
        };
        if self
            .selected
            .is_some_and(|pid| !rows.iter().any(|row| row.pid == pid))
        {
            self.selected.set(None);
        }
        let snapshot = Rc::new(Snapshot {
            reading: Rc::clone(&self.reading),
            rows,
            scale,
        });
        self.row_order
            .set(RowOrder::new(Rc::clone(&snapshot), glides));
        self.table.set(snapshot);
        self.refocus();
    }

    fn refocus(&mut self) {
        let focus = self
            .selected
            .and_then(|pid| self.table.peek().rows.iter().position(|row| row.pid == pid));
        self.focus.set(focus);
    }
}

pub(crate) fn search_field(processes: &Processes) -> impl View {
    text_field(&processes.search)
        .placeholder(tr!("tasks.search"))
        .claims_focus(*processes.search_focused)
        .on_blur(Processes::search_blurred)
        .on_change(|processes: &mut Processes| processes.search())
        .control_size(ControlSize::Small)
        .width(SEARCH)
        .shrink(1.0)
        .min_width(SEARCH_FLOOR)
}

pub(crate) fn view(processes: &Processes) -> impl View {
    let selected = *processes.selected;
    let focus = *processes.focus;
    let grid = Grid {
        sizings: (*processes.sizings).clone(),
        titles: processes
            .sizings
            .iter()
            .map(|sizing| sizing.column.title().to_string())
            .collect(),
        table: processes.table.clone(),
        snapshot: Rc::clone(&processes.row_order.snapshot),
        glides: processes.row_order.glides,
        order: processes.row_order.order,
        focus,
        selected,
        sorting: *processes.sorting,
        tree: *processes.mode == Mode::Grouped,
    };

    let mode = *processes.mode;
    let open = Handler::new(|processes: &mut Processes, at: Point| processes.open_menu(at));
    column! {
        row! {
            spacer().width(0.0).fill_height().build().keyed((MARK, mode));
            context_area(grid.view(), open).grow(1.0).fill_height();
        }
        .grow(1.0)
        .fill_width();
        if let Some((pid, at)) = *processes.menu {
            commands(mode, pid, at);
        }
    }
    .grow(1.0)
}

fn commands(mode: Mode, pid: i32, at: Point) -> Element {
    menu()
        .items([
            menu_item(tr!("tasks.end-task"))
                .icon(MaterialSymbol::CLOSE)
                .on_press(|processes: &mut Processes| processes.end_task(tasks_process::terminate)),
            menu_item(tr!("tasks.kill-task"))
                .icon(MaterialSymbol::STOP_CIRCLE)
                .on_press(|processes: &mut Processes| processes.end_task(tasks_process::kill)),
        ])
        .width(MENU)
        .on_dismiss(|processes: &mut Processes| processes.dismiss())
        .build()
        .keyed(pid)
        .anchored(Anchor::new((MARK, mode), AnchorEdge::BesideTop).offset(at.x, at.y))
}

impl std::fmt::Debug for Processes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Processes")
            .field("mode", &self.mode)
            .field("rows", &self.row_order.snapshot.rows.len())
            .field("selected", &self.selected)
            .finish_non_exhaustive()
    }
}

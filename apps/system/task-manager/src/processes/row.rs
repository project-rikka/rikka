use std::borrow::Cow;
use std::cell::OnceCell;
use std::collections::HashSet;

use rikka_format::format_bytes;
use rikka_i18n::tr;
use rikka_ui::MaterialSymbol;
use tasks_process::{Category, Group, Process, State, Totals};

use crate::format;
use crate::processes::column::{Column, Sorting};

const SECTION: u64 = 1 << 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RowKind {
    Section(Category),
    Leader { expandable: bool, expanded: bool },
    Member,
}

#[derive(Clone, Debug)]
pub(crate) struct Cell {
    pub(crate) text: Cow<'static, str>,
    pub(crate) heat: f32,
}

impl Cell {
    fn plain(text: impl Into<Cow<'static, str>>) -> Cell {
        Cell {
            text: text.into(),
            heat: 0.0,
        }
    }

    fn measured(text: String, heat: f32) -> Cell {
        Cell {
            text: Cow::Owned(text),
            heat,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum RowPosition {
    Leader(usize),
    Member(usize, usize),
}

#[derive(Clone, Debug)]
pub(crate) struct Row {
    pub(crate) pid: i32,
    pub(crate) kind: RowKind,
    pub(crate) icon: Option<MaterialSymbol>,
    pub(crate) label: String,
    at: Option<RowPosition>,
    metrics: Totals,
    cells: [OnceCell<Cell>; Column::COUNT],
}

impl Row {
    fn new(pid: i32, kind: RowKind, icon: Option<MaterialSymbol>, label: String) -> Row {
        Row {
            pid,
            kind,
            icon,
            label,
            at: None,
            metrics: Totals::default(),
            cells: Default::default(),
        }
    }

    fn placed(mut self, at: RowPosition, metrics: Totals) -> Row {
        self.at = Some(at);
        self.metrics = metrics;
        self
    }

    pub(crate) fn selectable(&self) -> bool {
        !matches!(self.kind, RowKind::Section(_))
    }

    pub(crate) fn name(&self) -> u64 {
        match self.kind {
            RowKind::Section(category) => SECTION | category as u64,
            _ => u64::from(self.pid.cast_unsigned()),
        }
    }

    pub(crate) fn indent(&self) -> f32 {
        match self.kind {
            RowKind::Member => 1.0,
            _ => 0.0,
        }
    }

    pub(crate) fn cell(&self, groups: &[Group], column: Column, scale: Scale) -> Option<&Cell> {
        let cells = &self.cells[column as usize];
        if let Some(cell) = cells.get() {
            return Some(cell);
        }
        let cell = self.write(groups, column, scale)?;
        Some(cells.get_or_init(|| cell))
    }

    fn write(&self, groups: &[Group], column: Column, scale: Scale) -> Option<Cell> {
        let process = self.process(groups)?;
        let metrics = &self.metrics;
        Some(match column {
            Column::Name => Cell::plain(""),
            Column::Pid => Cell::plain(process.pid.to_string()),
            Column::Status => Cell::plain(state(process.state)),
            Column::User => Cell::plain(process.user.clone()),
            Column::Command => Cell::plain(command(process)),
            Column::Threads => Cell::plain(metrics.threads.to_string()),
            Column::Cpu => Cell::measured(format::percent(metrics.cpu, 1), metrics.cpu),
            Column::Memory => match metrics.memory {
                Some(used) => Cell::measured(format_bytes(used), used as f32 / scale.memory),
                None => Cell::plain(format::UNKNOWN),
            },
            Column::Disk => Cell::measured(format::rate(metrics.disk), metrics.disk / scale.disk),
            Column::Gpu => Cell::measured(format::percent(metrics.gpu, 1), metrics.gpu),
        })
    }

    fn process<'a>(&self, groups: &'a [Group]) -> Option<&'a Process> {
        match self.at? {
            RowPosition::Leader(group) => Some(groups.get(group)?.leader()),
            RowPosition::Member(group, member) => groups.get(group)?.members().get(member),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Scale {
    pub(crate) memory: f32,
    pub(crate) disk: f32,
}

impl Scale {
    pub(crate) fn new(groups: &[Group], total_memory: u64) -> Self {
        let largest = |value: fn(&Group) -> f32| groups.iter().map(value).fold(0.0_f32, f32::max);
        Self {
            memory: largest(|group| group.totals().memory.unwrap_or_default() as f32)
                .max(total_memory as f32 * 0.05),
            disk: largest(|group| group.totals().disk).max(5_000_000.0),
        }
    }
}

pub(crate) struct Builder<'a> {
    sorting: Sorting,
    search: String,
    pid: Option<i32>,
    expanded: &'a HashSet<i32>,
}

impl<'a> Builder<'a> {
    pub(crate) fn new(sorting: Sorting, search: &str, expanded: &'a HashSet<i32>) -> Self {
        Self {
            sorting,
            search: search.to_lowercase(),
            pid: search
                .parse()
                .ok()
                .filter(|pid: &i32| pid.to_string() == search),
            expanded,
        }
    }

    pub(crate) fn grouped(&self, groups: &[Group]) -> Vec<Row> {
        let mut rows = Vec::new();
        for category in [Category::App, Category::Background, Category::System] {
            let mut matched: Vec<(usize, &Group)> = groups
                .iter()
                .enumerate()
                .filter(|(_, group)| group.category() == category)
                .filter(|(_, group)| self.matches_group(group))
                .collect();
            if matched.is_empty() {
                continue;
            }
            matched.sort_by(|(_, left), (_, right)| {
                self.sorting
                    .apply(self.sorting.column.compare_groups(left, right))
            });
            rows.push(section(category, &matched));
            for (at, group) in matched {
                let expanded = self.expanded.contains(&group.leader().pid);
                rows.push(self.leader(at, group, expanded));
                if !expanded {
                    continue;
                }
                let mut members: Vec<(usize, &Process)> = group
                    .members()
                    .iter()
                    .enumerate()
                    .filter(|(_, process)| self.matches(process))
                    .collect();
                members.sort_by(|(_, left), (_, right)| {
                    self.sorting.apply(self.sorting.column.compare(left, right))
                });
                rows.extend(members.into_iter().map(|(member, process)| {
                    self.member(RowPosition::Member(at, member), process)
                }));
            }
        }
        rows
    }

    pub(crate) fn flat(&self, groups: &[Group]) -> Vec<Row> {
        let mut processes: Vec<(RowPosition, &Process, Category)> = groups
            .iter()
            .enumerate()
            .flat_map(|(at, group)| {
                let leader = std::iter::once((RowPosition::Leader(at), group.leader()));
                let members = group
                    .members()
                    .iter()
                    .enumerate()
                    .map(move |(member, process)| (RowPosition::Member(at, member), process));
                leader
                    .chain(members)
                    .map(move |(place, process)| (place, process, group.category()))
            })
            .filter(|(_, process, _)| self.matches(process))
            .collect();
        processes.sort_by(|(_, left, _), (_, right, _)| {
            self.sorting.apply(self.sorting.column.compare(left, right))
        });
        processes
            .into_iter()
            .map(|(at, process, category)| {
                let kind = RowKind::Leader {
                    expandable: false,
                    expanded: false,
                };
                Row::new(
                    process.pid,
                    kind,
                    Some(icon(category)),
                    process.name.clone(),
                )
                .placed(at, Totals::from_process(process))
            })
            .collect()
    }

    fn leader(&self, at: usize, group: &Group, expanded: bool) -> Row {
        let leader = group.leader();
        let label = if group.count() > 1 {
            format!("{} ({})", leader.name, group.count())
        } else {
            leader.name.clone()
        };
        let kind = RowKind::Leader {
            expandable: group.count() > 1,
            expanded,
        };
        Row::new(leader.pid, kind, Some(icon(group.category())), label)
            .placed(RowPosition::Leader(at), group.totals())
    }

    fn member(&self, at: RowPosition, process: &Process) -> Row {
        Row::new(process.pid, RowKind::Member, None, process.name.clone())
            .placed(at, Totals::from_process(process))
    }

    fn matches_group(&self, group: &Group) -> bool {
        self.search.is_empty() || group.processes().any(|process| self.matches(process))
    }

    fn matches(&self, process: &Process) -> bool {
        self.search.is_empty()
            || process.matches_search(&self.search)
            || self.pid == Some(process.pid)
    }
}

fn section(category: Category, groups: &[(usize, &Group)]) -> Row {
    let processes: usize = groups.iter().map(|(_, group)| group.count()).sum();
    let name = match category {
        Category::App => tr!("tasks.section.apps", count = processes),
        Category::Background => tr!("tasks.section.background", count = processes),
        Category::System => tr!("tasks.section.system", count = processes),
    };
    Row::new(0, RowKind::Section(category), None, name.to_string())
}

fn icon(category: Category) -> MaterialSymbol {
    match category {
        Category::App => MaterialSymbol::APPS,
        Category::Background => MaterialSymbol::TOKEN,
        Category::System => MaterialSymbol::SETTINGS,
    }
}

fn state(state: State) -> String {
    match state {
        State::Running => tr!("tasks.state.running"),
        State::Sleeping => tr!("tasks.state.sleeping"),
        State::Waiting => tr!("tasks.state.waiting"),
        State::Stopped => tr!("tasks.state.stopped"),
        State::Zombie => tr!("tasks.state.zombie"),
        State::Idle => tr!("tasks.state.idle"),
    }
    .to_string()
}

fn command(process: &Process) -> String {
    match process.is_kernel() {
        true => tr!("tasks.kernel-thread").to_string(),
        false => process.command.clone(),
    }
}

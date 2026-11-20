use std::cmp::Ordering;

use rikka_i18n::tr;
use rikka_ui::{Label, TextAlign};
use tasks_process::{Group, Process};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Column {
    Name,
    Pid,
    Status,
    User,
    Cpu,
    Memory,
    Disk,
    Gpu,
    Threads,
    Command,
}

impl Column {
    pub(crate) const COUNT: usize = 10;
    pub(crate) fn title(self) -> Label {
        match self {
            Self::Name => tr!("tasks.column.name"),
            Self::Pid => tr!("tasks.column.pid"),
            Self::Status => tr!("tasks.column.status"),
            Self::User => tr!("tasks.column.user"),
            Self::Cpu => tr!("tasks.column.cpu"),
            Self::Memory => tr!("tasks.column.memory"),
            Self::Disk => tr!("tasks.column.disk"),
            Self::Gpu => tr!("tasks.column.gpu"),
            Self::Threads => tr!("tasks.column.threads"),
            Self::Command => tr!("tasks.column.command"),
        }
    }

    pub(crate) fn width(self) -> f32 {
        match self {
            Self::Name | Self::Command => 260.0,
            Self::Status => 96.0,
            Self::User | Self::Memory => 108.0,
            Self::Pid | Self::Threads => 76.0,
            Self::Cpu | Self::Gpu => 84.0,
            Self::Disk => 110.0,
        }
    }

    pub(crate) fn align(self) -> TextAlign {
        match self {
            Self::Name | Self::Status | Self::User | Self::Command => TextAlign::Left,
            _ => TextAlign::Right,
        }
    }

    pub(crate) fn descends_first(self) -> bool {
        !matches!(self, Self::Name | Self::Status | Self::User | Self::Command)
    }

    pub(crate) fn compare_groups(self, left: &Group, right: &Group) -> Ordering {
        let (totals, others) = (left.totals(), right.totals());
        match self {
            Self::Cpu => totals.cpu.total_cmp(&others.cpu),
            Self::Memory => totals.memory.cmp(&others.memory),
            Self::Disk => totals.disk.total_cmp(&others.disk),
            Self::Gpu => totals.gpu.total_cmp(&others.gpu),
            Self::Threads => totals.threads.cmp(&others.threads),
            _ => self.compare(left.leader(), right.leader()),
        }
    }

    pub(crate) fn compare(self, left: &Process, right: &Process) -> Ordering {
        match self {
            Self::Name => case_folded(&left.name).cmp(case_folded(&right.name)),
            Self::Pid => left.pid.cmp(&right.pid),
            Self::Status => (left.state as u8).cmp(&(right.state as u8)),
            Self::User => left.user.cmp(&right.user),
            Self::Cpu => left.cpu.total_cmp(&right.cpu),
            Self::Memory => left.memory.cmp(&right.memory),
            Self::Disk => left.disk.total_cmp(&right.disk),
            Self::Gpu => left.gpu.total_cmp(&right.gpu),
            Self::Threads => left.threads.cmp(&right.threads),
            Self::Command => left.command.cmp(&right.command),
        }
        .then_with(|| left.pid.cmp(&right.pid))
    }
}

fn case_folded(name: &str) -> impl Iterator<Item = char> + '_ {
    name.chars().flat_map(char::to_lowercase)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Sorting {
    pub(crate) column: Column,
    pub(crate) descending: bool,
}

impl Sorting {
    pub(crate) fn toggled(self, column: Column) -> Self {
        if self.column == column {
            return Self {
                column,
                descending: !self.descending,
            };
        }
        Self {
            column,
            descending: column.descends_first(),
        }
    }

    pub(crate) fn apply(self, ordering: Ordering) -> Ordering {
        if self.descending {
            ordering.reverse()
        } else {
            ordering
        }
    }
}

impl Default for Sorting {
    fn default() -> Self {
        Self {
            column: Column::Cpu,
            descending: true,
        }
    }
}

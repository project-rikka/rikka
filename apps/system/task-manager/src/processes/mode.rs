use crate::processes::column::Column;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Mode {
    Grouped,
    Flat,
}

impl Mode {
    pub(crate) fn columns(self) -> Vec<Column> {
        match self {
            Self::Grouped => vec![
                Column::Name,
                Column::Status,
                Column::Cpu,
                Column::Memory,
                Column::Disk,
                Column::Gpu,
            ],
            Self::Flat => vec![
                Column::Name,
                Column::Pid,
                Column::Status,
                Column::User,
                Column::Cpu,
                Column::Memory,
                Column::Disk,
                Column::Gpu,
                Column::Threads,
                Column::Command,
            ],
        }
    }
}

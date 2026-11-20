use std::fmt;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Sort {
    #[default]
    Time,
    SnoozeTime,
    Size,
    Id,
}

impl Sort {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Sort::Time => "Time",
            Sort::SnoozeTime => "SnoozeTime",
            Sort::Size => "Size",
            Sort::Id => "ID",
        }
    }
}

impl fmt::Display for Sort {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

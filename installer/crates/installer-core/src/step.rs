#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Step {
    Restore,
    Language,
    Keyboard,
    Timezone,
    Disk,
    Account,
    System,
    Apps,
    Summary,
    Install,
}

impl Step {
    pub const ALL: &'static [Step] = &[
        Step::Restore,
        Step::Language,
        Step::Keyboard,
        Step::Timezone,
        Step::Disk,
        Step::Account,
        Step::System,
        Step::Apps,
        Step::Summary,
        Step::Install,
    ];

    pub const POINT_OF_NO_RETURN: Step = Step::Install;

    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Restore => "Start",
            Self::Language => "Language",
            Self::Keyboard => "Keyboard",
            Self::Timezone => "Time zone",
            Self::Disk => "Disk",
            Self::Account => "Account",
            Self::System => "System",
            Self::Apps => "Apps",
            Self::Summary => "Summary",
            Self::Install => "Install",
        }
    }

    #[must_use]
    pub fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|step| *step == self)
            .unwrap_or_default()
    }

    #[must_use]
    pub fn next(self) -> Option<Self> {
        Self::ALL.get(self.index() + 1).copied()
    }

    #[must_use]
    pub fn previous(self) -> Option<Self> {
        self.index()
            .checked_sub(1)
            .and_then(|index| Self::ALL.get(index))
            .copied()
    }
}

use installer_core::Answers;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Partition,
    Encrypt,
    Format,
    Mount,
    Scan,
    Configure,
    Install,
    Finish,
}

impl Stage {
    #[must_use]
    pub fn plan(answers: &Answers) -> Vec<Stage> {
        [
            Stage::Partition,
            Stage::Encrypt,
            Stage::Format,
            Stage::Mount,
            Stage::Scan,
            Stage::Configure,
            Stage::Install,
            Stage::Finish,
        ]
        .into_iter()
        .filter(|stage| *stage != Stage::Encrypt || answers.encrypt)
        .collect()
    }

    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Partition => "Partition the disk",
            Self::Encrypt => "Encrypt the root",
            Self::Format => "Make the filesystems",
            Self::Mount => "Mount",
            Self::Scan => "Scan the hardware",
            Self::Configure => "Write the configuration",
            Self::Install => "Install NixOS",
            Self::Finish => "Finish",
        }
    }
}

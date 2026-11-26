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
    Handover,
    Finish,
}

impl Stage {
    #[must_use]
    pub fn plan(answers: &Answers) -> Vec<Stage> {
        let hands_over = answers
            .restored
            .as_ref()
            .is_some_and(|restored| restored.pending.is_some());
        [
            Stage::Partition,
            Stage::Encrypt,
            Stage::Format,
            Stage::Mount,
            Stage::Scan,
            Stage::Configure,
            Stage::Install,
            Stage::Handover,
            Stage::Finish,
        ]
        .into_iter()
        .filter(|stage| match stage {
            Stage::Encrypt => answers.encrypt,
            Stage::Handover => hands_over,
            _ => true,
        })
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
            Self::Handover => "Hand over the backup",
            Self::Finish => "Finish",
        }
    }
}

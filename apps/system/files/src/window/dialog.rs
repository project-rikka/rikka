use files_convert::Target;
use files_core::Location;
use files_drives::VolumeId;
use rikka_ui::TextDocument;

#[derive(Clone, Debug)]
pub(crate) enum Dialog {
    Properties {
        heading: String,
        lines: Vec<(String, String)>,
    },
    ConfirmDelete {
        items: Vec<Location>,
    },
    ConfirmRun {
        program: Location,
    },
    Password {
        asking: Asking,
        typed: TextDocument,
        wrong: bool,
    },
    Compress(Compressing),
    Convert(Converting),
}

#[derive(Clone, Debug)]
pub(crate) struct Converting {
    pub(crate) sources: Vec<Location>,
    pub(crate) targets: Vec<Target>,
    pub(crate) target: usize,
    pub(crate) quality: usize,
    pub(crate) size: usize,
}

impl Converting {
    pub(crate) fn chosen(&self) -> Target {
        self.targets[self.target]
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Compressing {
    pub(crate) sources: Vec<Location>,
    pub(crate) into: Location,
    pub(crate) name: TextDocument,
    pub(crate) format: usize,
    pub(crate) compression: usize,
    pub(crate) password: TextDocument,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Asking {
    Volume { id: VolumeId, name: String },
    Browse { archive: Location },
    Extract { archive: Location, beside: Location },
}

impl Asking {
    pub(crate) fn name(&self) -> String {
        match self {
            Self::Volume { name, .. } => name.clone(),
            Self::Browse { archive } | Self::Extract { archive, .. } => archive.name(),
        }
    }

    pub(crate) fn is_volume(&self) -> bool {
        matches!(self, Self::Volume { .. })
    }
}

use rikka_nixos_layout::{Module, Modules};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Action {
    #[default]
    Switch,
    Boot,
    Test,
    Build,
}

impl Action {
    pub const ALL: [Self; 4] = [Self::Switch, Self::Boot, Self::Test, Self::Build];

    #[must_use]
    pub fn argument(self) -> &'static str {
        match self {
            Self::Switch => "switch",
            Self::Boot => "boot",
            Self::Test => "test",
            Self::Build => "dry-activate",
        }
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Switch => "switch",
            Self::Boot => "boot",
            Self::Test => "test",
            Self::Build => "build",
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|action| action.name() == name)
    }

    #[must_use]
    pub fn activates(self) -> bool {
        self != Self::Build
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleFile {
    pub module: Module,
    pub base: String,
    pub contents: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    Apply {
        files: Vec<ModuleFile>,
        action: Action,
    },
    Rollback {
        generation: u32,
    },
    Update {
        action: Action,
    },
}

impl Request {
    #[must_use]
    pub fn apply(saved: &Modules, edited: &Modules, action: Action) -> Self {
        let files = Module::ALL
            .into_iter()
            .filter(|module| edited.get(*module) != saved.get(*module))
            .map(|module| ModuleFile {
                module,
                base: saved.get(module).source().to_owned(),
                contents: edited.get(module).source().to_owned(),
            })
            .collect();
        Self::Apply { files, action }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeState {
    Untouched,
    Modified,
}

impl ChangeState {
    #[must_use]
    pub fn from_unchanged(unchanged: bool) -> Self {
        match unchanged {
            true => Self::Untouched,
            false => Self::Modified,
        }
    }

    #[must_use]
    pub fn is_untouched(self) -> bool {
        self == Self::Untouched
    }
}

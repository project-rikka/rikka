use std::path::{Path, PathBuf};

const RIKKA: &str = ".rikka";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Scope {
    Project,
    User,
}

impl Scope {
    pub const PARTICULAR_FIRST: [Scope; 2] = [Scope::Project, Scope::User];

    pub const GENERAL_FIRST: [Scope; 2] = [Scope::User, Scope::Project];

    #[must_use]
    pub fn root(self, project: Option<&Path>) -> Option<PathBuf> {
        match self {
            Scope::Project => Some(project?.to_path_buf()),
            Scope::User => rikka_dirs::home().map(Path::to_path_buf),
        }
    }

    #[must_use]
    pub fn rikka(self, project: Option<&Path>) -> Option<PathBuf> {
        Some(self.root(project)?.join(RIKKA))
    }
}

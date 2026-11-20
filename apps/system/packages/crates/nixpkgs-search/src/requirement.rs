#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Setting {
    Enabled,
    Value(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Requirement {
    Option {
        path: String,
        setting: Setting,
        description: String,
    },
    Group(String),
    Unfree,
}

impl Requirement {
    #[must_use]
    pub fn key(&self) -> String {
        match self {
            Self::Option { path, .. } => path.clone(),
            Self::Group(name) => format!("group:{name}"),
            Self::Unfree => "unfree".to_owned(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
    pub requirement: Requirement,
    pub recommended: bool,
}

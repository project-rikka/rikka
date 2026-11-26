use std::fmt;
use std::str::FromStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Scope {
    Machine,
    User,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    Default,
    System,
    Declared(Scope),
    User,
    Locked(Scope),
}

impl Source {
    pub const ALL: [Source; 7] = [
        Source::Default,
        Source::System,
        Source::Declared(Scope::Machine),
        Source::Declared(Scope::User),
        Source::User,
        Source::Locked(Scope::Machine),
        Source::Locked(Scope::User),
    ];

    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::System => "system",
            Self::Declared(Scope::Machine) => "declared-machine",
            Self::Declared(Scope::User) => "declared-user",
            Self::User => "user",
            Self::Locked(Scope::Machine) => "locked-machine",
            Self::Locked(Scope::User) => "locked-user",
        }
    }

    #[must_use]
    pub fn is_locked(self) -> bool {
        matches!(self, Self::Locked(_))
    }

    #[must_use]
    pub fn is_declared(self) -> bool {
        matches!(self, Self::Declared(_) | Self::Locked(_))
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.slug())
    }
}

impl FromStr for Source {
    type Err = String;

    fn from_str(slug: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|source| source.slug() == slug)
            .ok_or_else(|| format!("`{slug}` is not a settings layer"))
    }
}

impl serde::Serialize for Source {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.slug())
    }
}

impl<'de> serde::Deserialize<'de> for Source {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let slug = String::deserialize(deserializer)?;
        slug.parse().map_err(serde::de::Error::custom)
    }
}

const MIN_COMMIT_LENGTH: usize = 7;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pin {
    channel: String,
    version: Option<Version>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Version {
    release: String,
    commit: String,
}

impl Pin {
    #[must_use]
    pub fn new(channel: impl Into<String>, nixos_version: &str) -> Self {
        Self {
            channel: channel.into(),
            version: Version::parse(nixos_version.trim()),
        }
    }

    #[must_use]
    pub fn channel(&self) -> &str {
        &self.channel
    }

    #[must_use]
    pub fn release(&self) -> Option<&str> {
        self.version
            .as_ref()
            .map(|version| version.release.as_str())
    }

    #[must_use]
    pub fn commit(&self) -> Option<&str> {
        self.version.as_ref().map(|version| version.commit.as_str())
    }
}

impl Version {
    fn parse(version: &str) -> Option<Self> {
        let mut parts = version.split('.');
        let major = parts.next().filter(|major| is_numeric(major))?;
        let minor: String = parts
            .next()?
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        if minor.is_empty() {
            return None;
        }
        let commit = version
            .rsplit('.')
            .next()
            .filter(|commit| is_commit(commit))?;
        Some(Self {
            release: format!("{major}.{minor}"),
            commit: commit.to_ascii_lowercase(),
        })
    }
}

fn is_numeric(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|ch| ch.is_ascii_digit())
}

pub(crate) fn is_commit(text: &str) -> bool {
    text.len() >= MIN_COMMIT_LENGTH && text.chars().all(|ch| ch.is_ascii_hexdigit())
}

pub(crate) fn same_commit(one: &str, other: &str) -> bool {
    is_commit(one) && is_commit(other) && (one.starts_with(other) || other.starts_with(one))
}

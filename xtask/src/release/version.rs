use std::fmt;

use anyhow::{Context, Result, bail};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Bump {
    Patch,
    Minor,
    Major,
    Exact(Version),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Version {
    major: u32,
    minor: u32,
    patch: u32,
}

impl Version {
    pub(crate) fn parse(text: &str) -> Result<Version> {
        let parts: Vec<&str> = text.split('.').collect();
        let [major, minor, patch] = parts.as_slice() else {
            bail!("version `{text}` is not MAJOR.MINOR.PATCH");
        };
        let number = |part: &str| {
            part.parse::<u32>()
                .with_context(|| format!("version `{text}` is not MAJOR.MINOR.PATCH"))
        };
        Ok(Version {
            major: number(major)?,
            minor: number(minor)?,
            patch: number(patch)?,
        })
    }
}

impl fmt::Display for Version {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl Bump {
    pub(crate) fn apply(&self, subject: &str, published: Option<&str>) -> Result<Version> {
        let published = published.map(Version::parse).transpose()?;
        let next = match (self, published) {
            (Self::Exact(version), _) => *version,
            (_, None) => bail!("{subject} has no published version to bump; pass --version"),
            (Self::Patch, Some(current)) => Version {
                patch: current.patch + 1,
                ..current
            },
            (Self::Minor, Some(current)) => Version {
                minor: current.minor + 1,
                patch: 0,
                ..current
            },
            (Self::Major, Some(current)) => Version {
                major: current.major + 1,
                minor: 0,
                patch: 0,
            },
        };
        if let Some(current) = published
            && next < current
        {
            bail!("{subject} is published at {current}; {next} would go back");
        }
        Ok(next)
    }
}

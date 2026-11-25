use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer};

use crate::error::{Error, Result};

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Coordinate {
    pub group: String,
    pub artifact: String,
    pub version: String,
    pub classifier: Option<String>,
    pub extension: String,
}

impl Coordinate {
    #[must_use]
    pub fn path(&self) -> String {
        let file = match &self.classifier {
            Some(classifier) => format!(
                "{}-{}-{classifier}.{}",
                self.artifact, self.version, self.extension
            ),
            None => format!("{}-{}.{}", self.artifact, self.version, self.extension),
        };
        format!(
            "{}/{}/{}/{file}",
            self.group.replace('.', "/"),
            self.artifact,
            self.version
        )
    }

    #[must_use]
    pub fn identity(&self) -> (&str, &str, Option<&str>) {
        (&self.group, &self.artifact, self.classifier.as_deref())
    }
}

fn segment(text: &str) -> bool {
    !text.is_empty()
        && text != "."
        && text != ".."
        && text
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-_.+".contains(character))
}

impl FromStr for Coordinate {
    type Err = Error;

    fn from_str(name: &str) -> Result<Coordinate> {
        let invalid = || Error::Coordinate(name.to_owned());
        let (body, extension) = match name.split_once('@') {
            Some((body, extension)) => (body, extension),
            None => (name, "jar"),
        };
        let mut parts = body.split(':');
        let (Some(group), Some(artifact), Some(version), classifier, None) = (
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
        ) else {
            return Err(invalid());
        };
        let valid = group.split('.').all(segment)
            && segment(artifact)
            && segment(version)
            && segment(extension)
            && classifier.is_none_or(segment);
        if !valid {
            return Err(invalid());
        }
        Ok(Coordinate {
            group: group.to_owned(),
            artifact: artifact.to_owned(),
            version: version.to_owned(),
            classifier: classifier.map(str::to_owned),
            extension: extension.to_owned(),
        })
    }
}

impl fmt::Display for Coordinate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}:{}:{}",
            self.group, self.artifact, self.version
        )?;
        if let Some(classifier) = &self.classifier {
            write!(formatter, ":{classifier}")?;
        }
        if self.extension != "jar" {
            write!(formatter, "@{}", self.extension)?;
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for Coordinate {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Coordinate, D::Error> {
        let name = String::deserialize(deserializer)?;
        name.parse().map_err(serde::de::Error::custom)
    }
}

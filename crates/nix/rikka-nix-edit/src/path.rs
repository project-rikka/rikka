use std::fmt;
use std::str::FromStr;

use rnix::Root;
use rnix::ast::{Entry, Expr, HasEntry};

use crate::error::{Error, Result};
use crate::name;
use crate::syntax;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AttrPath(Vec<String>);

impl AttrPath {
    pub fn parse(text: &str) -> Result<Self> {
        let plain: Vec<String> = text.split('.').map(str::to_owned).collect();
        if plain.iter().all(|segment| name::is_identifier(segment)) {
            return Ok(Self(plain));
        }
        let refused = || Error::AttrPath(text.to_owned());
        let parsed = Root::parse(&format!("{{ {text} = null; }}"));
        if !parsed.errors().is_empty() {
            return Err(refused());
        }
        let Some(Expr::AttrSet(set)) = parsed.tree().expr() else {
            return Err(refused());
        };
        let mut entries = set.entries();
        let (Some(Entry::AttrpathValue(only)), None) = (entries.next(), entries.next()) else {
            return Err(refused());
        };
        only.attrpath()
            .and_then(|attrpath| syntax::keys(&attrpath))
            .map(Self)
            .ok_or_else(refused)
    }

    pub fn new<S: AsRef<str>>(segments: impl IntoIterator<Item = S>) -> Result<Self> {
        let segments: Vec<String> = segments
            .into_iter()
            .map(|segment| segment.as_ref().to_owned())
            .collect();
        match segments.is_empty() {
            true => Err(Error::AttrPath(String::new())),
            false => Ok(Self(segments)),
        }
    }

    pub(crate) fn from_segments(segments: Vec<String>) -> Self {
        Self(segments)
    }

    #[must_use]
    pub fn segments(&self) -> &[String] {
        &self.0
    }

    #[must_use]
    pub fn head(&self) -> &str {
        &self.0[0]
    }

    #[must_use]
    pub fn is_referable(&self) -> bool {
        name::is_identifier(self.head())
    }

    #[must_use]
    pub fn join(&self, rest: &Self) -> Self {
        Self(self.0.iter().chain(&rest.0).cloned().collect())
    }

    #[must_use]
    pub fn strip_prefix(&self, prefix: &Self) -> Option<Self> {
        let rest = self.0.strip_prefix(prefix.0.as_slice())?;
        (!rest.is_empty()).then(|| Self(rest.to_vec()))
    }
}

impl fmt::Display for AttrPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, segment) in self.0.iter().enumerate() {
            if index > 0 {
                f.write_str(".")?;
            }
            f.write_str(&name::attribute(segment))?;
        }
        Ok(())
    }
}

impl FromStr for AttrPath {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self> {
        Self::parse(text)
    }
}

pub trait IntoAttrPath {
    fn into_attr_path(self) -> Result<AttrPath>;
}

impl IntoAttrPath for AttrPath {
    fn into_attr_path(self) -> Result<AttrPath> {
        Ok(self)
    }
}

impl IntoAttrPath for &AttrPath {
    fn into_attr_path(self) -> Result<AttrPath> {
        Ok(self.clone())
    }
}

impl IntoAttrPath for &str {
    fn into_attr_path(self) -> Result<AttrPath> {
        AttrPath::parse(self)
    }
}

impl IntoAttrPath for &String {
    fn into_attr_path(self) -> Result<AttrPath> {
        AttrPath::parse(self)
    }
}

impl IntoAttrPath for String {
    fn into_attr_path(self) -> Result<AttrPath> {
        AttrPath::parse(&self)
    }
}

impl<S: AsRef<str>, const N: usize> IntoAttrPath for [S; N] {
    fn into_attr_path(self) -> Result<AttrPath> {
        AttrPath::new(self)
    }
}

impl<S: AsRef<str>> IntoAttrPath for &[S] {
    fn into_attr_path(self) -> Result<AttrPath> {
        AttrPath::new(self)
    }
}

impl IntoAttrPath for Vec<String> {
    fn into_attr_path(self) -> Result<AttrPath> {
        AttrPath::new(self)
    }
}

use std::fmt;

use crate::fourcc::Fourcc;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Modifier(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Format {
    pub fourcc: Fourcc,
    pub modifier: Modifier,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FormatSet(Box<[Format]>);

impl Modifier {
    pub const LINEAR: Self = Self(0);
    pub const INVALID: Self = Self(0x00ff_ffff_ffff_ffff);

    pub fn is_implicit(self) -> bool {
        self == Self::INVALID
    }
}

impl Format {
    pub const fn new(fourcc: Fourcc, modifier: Modifier) -> Self {
        Self { fourcc, modifier }
    }
}

impl FormatSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn contains(&self, format: Format) -> bool {
        self.0.binary_search(&format).is_ok()
    }

    pub fn supports(&self, fourcc: Fourcc) -> bool {
        !self.with_fourcc(fourcc).is_empty()
    }

    pub fn modifiers(&self, fourcc: Fourcc) -> impl Iterator<Item = Modifier> + '_ {
        self.with_fourcc(fourcc)
            .iter()
            .map(|format| format.modifier)
    }

    pub fn iter(&self) -> impl Iterator<Item = Format> + '_ {
        self.0.iter().copied()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn intersection(&self, other: &Self) -> Self {
        Self(
            self.iter()
                .filter(|format| other.contains(*format))
                .collect(),
        )
    }

    fn with_fourcc(&self, fourcc: Fourcc) -> &[Format] {
        let start = self.0.partition_point(|format| format.fourcc < fourcc);
        let end = start + self.0[start..].partition_point(|format| format.fourcc == fourcc);
        &self.0[start..end]
    }
}

impl FromIterator<Format> for FormatSet {
    fn from_iter<I: IntoIterator<Item = Format>>(formats: I) -> Self {
        let mut formats: Vec<Format> = formats.into_iter().collect();
        formats.sort_unstable();
        formats.dedup();
        Self(formats.into_boxed_slice())
    }
}

impl fmt::Debug for Modifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:#018x}", self.0)
    }
}

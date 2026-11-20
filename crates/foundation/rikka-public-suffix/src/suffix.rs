use crate::rules::{EXCEPTION, ICANN, NORMAL, PRIVATE, WILDCARD};
use crate::table::flags;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Section {
    Icann,
    Private,
    Unlisted,
}

impl Section {
    fn shift(self) -> Option<u8> {
        match self {
            Section::Icann => Some(ICANN),
            Section::Private => Some(PRIVATE),
            Section::Unlisted => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Suffix<'a> {
    text: &'a str,
    section: Section,
}

impl<'a> Suffix<'a> {
    #[must_use]
    pub fn as_str(&self) -> &'a str {
        self.text
    }

    #[must_use]
    pub fn section(&self) -> Section {
        self.section
    }

    #[must_use]
    pub fn is_listed(&self) -> bool {
        self.section != Section::Unlisted
    }
}

#[must_use]
pub fn suffix(domain: &str) -> Option<Suffix<'_>> {
    let labels = Labels::of(domain)?;
    Some(
        labels
            .prevailing(&[Section::Icann, Section::Private])
            .unwrap_or_else(|| Suffix {
                text: labels.last(1),
                section: Section::Unlisted,
            }),
    )
}

#[must_use]
pub fn icann_suffix(domain: &str) -> Option<Suffix<'_>> {
    Labels::of(domain)?.prevailing(&[Section::Icann])
}

struct Labels<'a> {
    domain: &'a str,
    starts: Vec<usize>,
}

impl<'a> Labels<'a> {
    fn of(domain: &'a str) -> Option<Labels<'a>> {
        if domain.split('.').any(str::is_empty) {
            return None;
        }
        let starts = std::iter::once(0)
            .chain(domain.match_indices('.').map(|(at, _)| at + 1))
            .collect();
        Some(Labels { domain, starts })
    }

    fn count(&self) -> usize {
        self.starts.len()
    }

    fn last(&self, count: usize) -> &'a str {
        &self.domain[self.starts[self.count() - count]..]
    }

    fn prevailing(&self, sections: &[Section]) -> Option<Suffix<'a>> {
        let has = |count: usize, kind: u8| {
            let found = flags(self.last(count));
            sections.iter().copied().find(|section| {
                section
                    .shift()
                    .is_some_and(|shift| found & (kind << shift) != 0)
            })
        };
        let exception = (2..=self.count()).rev().find_map(|count| {
            has(count, EXCEPTION).map(|section| Suffix {
                text: self.last(count - 1),
                section,
            })
        });
        exception.or_else(|| {
            (1..=self.count()).rev().find_map(|count| {
                let section = has(count, NORMAL)
                    .or_else(|| (count > 1).then(|| has(count - 1, WILDCARD)).flatten())?;
                Some(Suffix {
                    text: self.last(count),
                    section,
                })
            })
        })
    }
}

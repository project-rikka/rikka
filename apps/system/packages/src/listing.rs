use std::collections::HashMap;
use std::sync::Arc;

use nixpkgs_search::{Catalog, Id, Package};

use crate::mark::Mark;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Row {
    Cataloged(Id),
    Described(Arc<Package>),
    Unknown(Arc<str>),
}

pub(crate) type Rows = Arc<Vec<Row>>;

pub(crate) type Marks = Arc<HashMap<String, Mark>>;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Line<'a> {
    pub(crate) attribute: &'a str,
    pub(crate) name: &'a str,
    pub(crate) version: &'a str,
    pub(crate) description: &'a str,
    pub(crate) set: &'a str,
    pub(crate) unfree: bool,
    pub(crate) mark: Mark,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Listing {
    catalog: Option<Arc<Catalog>>,
    rows: Rows,
    marks: Marks,
}

impl Listing {
    pub(crate) fn new(catalog: Option<Arc<Catalog>>, rows: Rows, marks: Marks) -> Self {
        Self {
            catalog,
            rows,
            marks,
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.rows.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub(crate) fn has_same_rows(&self, other: &Listing) -> bool {
        Arc::ptr_eq(&self.rows, &other.rows) || self.rows == other.rows
    }

    pub(crate) fn position(&self, attribute: &str) -> Option<usize> {
        let cataloged = self
            .catalog
            .as_ref()
            .and_then(|catalog| catalog.find(attribute))
            .map(Row::Cataloged);
        match cataloged {
            Some(row) => self.rows.iter().position(|candidate| *candidate == row),
            None => (0..self.len()).find(|&at| self.attribute(at) == Some(attribute)),
        }
    }

    pub(crate) fn attribute(&self, at: usize) -> Option<&str> {
        Some(match self.rows.get(at)? {
            Row::Cataloged(id) => self.catalog.as_ref()?.entry(*id).attribute,
            Row::Described(package) => &package.attribute,
            Row::Unknown(attribute) => attribute,
        })
    }

    pub(crate) fn line(&self, at: usize) -> Option<Line<'_>> {
        let line = match self.rows.get(at)? {
            Row::Cataloged(id) => {
                let entry = self.catalog.as_ref()?.entry(*id);
                Line {
                    attribute: entry.attribute,
                    name: entry.name,
                    version: entry.version,
                    description: entry.description,
                    set: entry.set().unwrap_or_default(),
                    unfree: entry.unfree,
                    mark: Mark::default(),
                }
            }
            Row::Described(package) => Line {
                attribute: &package.attribute,
                name: &package.name,
                version: &package.version,
                description: &package.description,
                set: package.set.as_deref().unwrap_or_default(),
                unfree: package.unfree,
                mark: Mark::default(),
            },
            Row::Unknown(attribute) => Line {
                attribute,
                name: "",
                version: "",
                description: "",
                set: "",
                unfree: false,
                mark: Mark::default(),
            },
        };
        Some(Line {
            name: match line.name {
                "" => line.attribute,
                name => name,
            },
            mark: self.marks.get(line.attribute).copied().unwrap_or_default(),
            ..line
        })
    }

    pub(crate) fn package(&self, at: usize) -> nixpkgs_search::Result<Option<Arc<Package>>> {
        match (self.rows.get(at), &self.catalog) {
            (Some(Row::Cataloged(id)), Some(catalog)) => Ok(Some(Arc::new(catalog.package(*id)?))),
            (Some(Row::Described(package)), _) => Ok(Some(Arc::clone(package))),
            _ => Ok(None),
        }
    }
}

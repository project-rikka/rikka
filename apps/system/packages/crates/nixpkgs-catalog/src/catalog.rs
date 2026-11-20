use std::cmp::Ordering;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use brotli_decompressor::Decompressor;
use rikka_types::CancelToken;

use crate::detail::Detail;
use crate::error::{Error, Result};
use crate::file::{self, Contents, Details, Span};
use crate::import;
use crate::package::Package;
use crate::search_index::{self, SearchIndex};

const BROTLI_BUFFER: usize = 1 << 16;
const READ_BUFFER: usize = 1 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Id(u32);

#[derive(Clone, Copy, Debug)]
pub struct Entry<'a> {
    pub attribute: &'a str,
    pub name: &'a str,
    pub version: &'a str,
    pub description: &'a str,
    pub unfree: bool,
}

impl<'a> Entry<'a> {
    #[must_use]
    pub fn set(&self) -> Option<&'a str> {
        self.attribute.rsplit_once('.').map(|(set, _)| set)
    }
}

pub struct Catalog {
    release: String,
    platforms: Vec<String>,
    text: String,
    spans: Vec<Span>,
    search_index: SearchIndex,
    details: Details,
}

impl Catalog {
    pub fn import(from: &Path, release: &str, into: &Path, cancel: &CancelToken) -> Result<Self> {
        let read_error = Error::read(from);
        let compressed = File::open(from).map_err(&read_error)?;
        let reader =
            BufReader::with_capacity(READ_BUFFER, Decompressor::new(compressed, BROTLI_BUFFER));
        let mut imported = import::read(reader, cancel)?;
        imported.records.sort_by_cached_key(|record| {
            (
                search_index::fold_case(&record.attribute),
                record.attribute.clone(),
            )
        });
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }

        file::write(into, release, &imported)?;
        Self::load(into)
    }

    pub fn load(path: &Path) -> Result<Self> {
        let Contents {
            release,
            platforms,
            text,
            spans,
            details,
        } = file::read(path)?;
        let mut catalog = Self {
            release,
            platforms,
            text,
            spans,
            search_index: SearchIndex::default(),
            details,
        };
        let search_index = SearchIndex::new(catalog.ids().map(|id| {
            let entry = catalog.entry(id);
            [entry.attribute, entry.name, entry.description]
        }));
        catalog.search_index = search_index;
        Ok(catalog)
    }

    #[must_use]
    pub fn release(&self) -> &str {
        &self.release
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.spans.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    pub fn ids(&self) -> impl ExactSizeIterator<Item = Id> + use<> {
        (0..self.count()).map(Id)
    }

    #[must_use]
    pub fn entry(&self, id: Id) -> Entry<'_> {
        let at = id.0 as usize;
        let span = &self.spans[at];
        let start = match at {
            0 => 0,
            _ => self.spans[at - 1].ends[3],
        };
        let field = |from: u32, to: u32| &self.text[from as usize..to as usize];
        Entry {
            attribute: field(start, span.ends[0]),
            name: field(span.ends[0], span.ends[1]),
            version: field(span.ends[1], span.ends[2]),
            description: field(span.ends[2], span.ends[3]),
            unfree: span.unfree,
        }
    }

    #[must_use]
    pub fn find(&self, attribute: &str) -> Option<Id> {
        let lowered = search_index::fold_case(attribute);
        let probe = (lowered.as_str(), attribute);
        let (mut low, mut high) = (0, self.count());
        while low < high {
            let middle = low + (high - low) / 2;
            let candidate = (
                self.search_index.attribute(middle),
                self.entry(Id(middle)).attribute,
            );
            match candidate.cmp(&probe) {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => high = middle,
                Ordering::Equal => return Some(Id(middle)),
            }
        }
        None
    }

    #[must_use]
    pub fn search(&self, text: &str) -> Vec<Id> {
        self.search_index.search(text).into_iter().map(Id).collect()
    }

    pub fn package(&self, id: Id) -> Result<Package> {
        let entry = self.entry(id);
        let bytes = self
            .details
            .read(self.spans[id.0 as usize].detail)
            .map_err(Error::Details)?;
        let detail: Detail = serde_json::from_slice(&bytes).map_err(Error::DetailsFormat)?;
        Ok(Package {
            attribute: entry.attribute.to_owned(),
            set: entry.set().map(str::to_owned),
            name: entry.name.to_owned(),
            version: entry.version.to_owned(),
            description: entry.description.to_owned(),
            about: detail.about,
            main_program: named(detail.main_program),
            outputs: detail.outputs,
            platforms: detail
                .platforms
                .into_iter()
                .filter_map(|at| self.platforms.get(usize::from(at)).cloned())
                .collect(),
            unfree: entry.unfree,
            licenses: detail.licenses,
            maintainers: detail.maintainers,
            teams: detail.teams,
            homepages: detail.homepages,
            position: named(detail.position),
        })
    }

    fn count(&self) -> u32 {
        u32::try_from(self.spans.len()).unwrap_or(u32::MAX)
    }
}

impl std::fmt::Debug for Catalog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Catalog")
            .field("release", &self.release)
            .field("packages", &self.spans.len())
            .finish_non_exhaustive()
    }
}

fn named(value: String) -> Option<String> {
    Some(value).filter(|value| !value.is_empty())
}

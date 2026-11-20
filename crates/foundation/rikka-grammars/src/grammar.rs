use std::fmt;
use std::path::Path;
use std::sync::OnceLock;

use tree_sitter as ts;

use crate::library::{self, Loaded};
use crate::table::ENTRIES;

pub(crate) struct Entry {
    name: &'static str,
    symbol: &'static str,
    aliases: &'static [&'static str],
    extensions: &'static [&'static str],
    filenames: &'static [&'static str],
    loaded: OnceLock<Option<Loaded>>,
}

impl Entry {
    pub(crate) const fn new(name: &'static str, symbol: &'static str) -> Entry {
        Entry {
            name,
            symbol,
            aliases: &[],
            extensions: &[],
            filenames: &[],
            loaded: OnceLock::new(),
        }
    }

    pub(crate) const fn aliases(mut self, aliases: &'static [&'static str]) -> Entry {
        self.aliases = aliases;
        self
    }

    pub(crate) const fn extensions(mut self, extensions: &'static [&'static str]) -> Entry {
        self.extensions = extensions;
        self
    }

    pub(crate) const fn filenames(mut self, filenames: &'static [&'static str]) -> Entry {
        self.filenames = filenames;
        self
    }
}

#[derive(Clone, Copy)]
pub struct Grammar(&'static Entry);

impl Grammar {
    #[must_use]
    pub fn named(name: &str) -> Option<Grammar> {
        let name = name.trim();
        if name.is_empty() {
            return None;
        }
        Grammar::all().find(|grammar| {
            grammar.0.name.eq_ignore_ascii_case(name)
                || grammar
                    .0
                    .aliases
                    .iter()
                    .any(|alias| alias.eq_ignore_ascii_case(name))
        })
    }

    #[must_use]
    pub fn for_path(path: impl AsRef<Path>) -> Option<Grammar> {
        let path = path.as_ref();
        let filename = path.file_name()?.to_str()?;
        let known = Grammar::all().find(|grammar| {
            grammar
                .0
                .filenames
                .iter()
                .any(|named| named.eq_ignore_ascii_case(filename))
        });
        if known.is_some() {
            return known;
        }
        let extension = path.extension()?.to_str()?;
        Grammar::all().find(|grammar| {
            grammar
                .0
                .extensions
                .iter()
                .any(|known| known.eq_ignore_ascii_case(extension))
        })
    }

    pub fn all() -> impl Iterator<Item = Grammar> {
        ENTRIES.iter().map(Grammar)
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        self.0.name
    }

    #[must_use]
    pub fn language(self) -> Option<&'static ts::Language> {
        self.0
            .loaded
            .get_or_init(|| library::load(self.0.name, self.0.symbol))
            .as_ref()
            .map(|loaded| &loaded.language)
    }
}

impl fmt::Debug for Grammar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.name)
    }
}

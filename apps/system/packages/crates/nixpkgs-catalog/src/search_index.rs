use memchr::memmem::Finder;

use crate::rank::{self, Fields};

const SEPARATOR: char = '\0';

#[derive(Default)]
pub(crate) struct SearchIndex {
    text: String,
    starts: Vec<u32>,
}

pub(crate) fn fold_case(text: &str) -> String {
    text.to_lowercase().replace(SEPARATOR, " ")
}

impl SearchIndex {
    pub(crate) fn new<'a>(entries: impl ExactSizeIterator<Item = [&'a str; 3]>) -> Self {
        let mut text = String::new();
        let mut starts = Vec::with_capacity(entries.len() + 1);
        for fields in entries {
            starts.push(offset(text.len()));
            for field in fields {
                text.push_str(&fold_case(field));
                text.push(SEPARATOR);
            }
        }
        starts.push(offset(text.len()));
        Self { text, starts }
    }

    pub(crate) fn attribute(&self, id: u32) -> &str {
        self.entry(id).split(SEPARATOR).next().unwrap_or_default()
    }

    pub(crate) fn search(&self, query: &str) -> Vec<u32> {
        let mut words: Vec<String> = query
            .split_whitespace()
            .map(fold_case)
            .filter(|word| !word.is_empty())
            .collect();
        words.sort_unstable_by(|one, other| {
            other.len().cmp(&one.len()).then_with(|| one.cmp(other))
        });
        words.dedup();
        let Some((first, rest)) = words.split_first() else {
            return (0..self.len()).collect();
        };

        let mut matches = self.matching(first);
        for word in rest {
            let finder = Finder::new(word.as_bytes());
            matches.retain(|&id| finder.find(self.entry(id).as_bytes()).is_some());
        }
        matches.sort_by_cached_key(|&id| rank::order(&words, &self.fields(id), id));
        matches
    }

    fn matching(&self, word: &str) -> Vec<u32> {
        let finder = Finder::new(word.as_bytes());
        let search_index = self.text.as_bytes();
        let mut matches = Vec::new();
        let mut from = 0usize;
        while let Some(at) = finder.find(&search_index[from..]) {
            let id = self
                .starts
                .partition_point(|&start| start as usize <= from + at)
                - 1;
            matches.push(offset(id));
            from = self.starts[id + 1] as usize;
        }
        matches
    }

    fn len(&self) -> u32 {
        offset(self.starts.len() - 1)
    }

    fn entry(&self, id: u32) -> &str {
        let id = id as usize;
        &self.text[self.starts[id] as usize..self.starts[id + 1] as usize]
    }

    fn fields(&self, id: u32) -> Fields<'_> {
        let mut fields = self.entry(id).split(SEPARATOR);
        Fields {
            attribute: fields.next().unwrap_or_default(),
            name: fields.next().unwrap_or_default(),
            description: fields.next().unwrap_or_default(),
        }
    }
}

fn offset(length: usize) -> u32 {
    u32::try_from(length).unwrap_or(u32::MAX)
}

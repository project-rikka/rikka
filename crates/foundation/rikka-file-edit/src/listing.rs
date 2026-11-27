use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::anchor::Tag;
use crate::files::Files;
use crate::text::Text;
use crate::text_file::MAX_BYTES;

const BOM: char = '\u{feff}';

#[derive(Clone, Debug)]
pub struct Listing {
    folders: Vec<String>,
    files: Vec<String>,
    excluded: Vec<String>,
}

impl Default for Listing {
    fn default() -> Listing {
        Listing::new()
    }
}

impl Listing {
    #[must_use]
    pub fn new() -> Listing {
        Listing {
            folders: vec![String::new()],
            files: Vec::new(),
            excluded: Vec::new(),
        }
    }

    #[must_use]
    pub fn folder(mut self, folder: impl Into<String>) -> Listing {
        let folder = folder.into();
        if !self.folders.contains(&folder) {
            self.folders.push(folder);
        }
        self
    }

    #[must_use]
    pub fn file(mut self, path: impl Into<String>) -> Listing {
        self.files.push(path.into());
        self
    }

    #[must_use]
    pub fn except(mut self, folder: impl Into<String>) -> Listing {
        self.excluded.push(folder.into());
        self
    }
}

impl Files {
    #[must_use]
    pub fn anchor(&self, listing: &Listing, output: &str) -> String {
        let mut sources = Sources::new(self, listing);
        let named: Vec<PathBuf> = listing
            .files
            .iter()
            .filter_map(|file| sources.locate(file, ""))
            .collect();

        let mut anchored = String::with_capacity(output.len() + output.len() / 8);
        for line in output.split_inclusive('\n') {
            let (body, end) = split_end(line);
            match sources.anchored(body, &named) {
                Some(body) => {
                    anchored.push_str(&body);
                    anchored.push_str(end);
                }
                None => anchored.push_str(line),
            }
        }
        anchored
    }
}

struct Sources<'a> {
    files: &'a Files,
    folders: &'a [String],
    excluded: Vec<PathBuf>,
    located: HashMap<PathBuf, Option<PathBuf>>,
    read: HashMap<PathBuf, Option<Text>>,
}

struct Row<'a> {
    number: usize,
    end: usize,
    content: &'a str,
}

impl<'a> Row<'a> {
    fn new(number: usize, end: usize, text: &'a str) -> Row<'a> {
        let content = match number {
            1 => text.strip_prefix(BOM).unwrap_or(text),
            _ => text,
        };
        Row {
            number,
            end,
            content,
        }
    }

    fn tagged(&self, body: &str) -> String {
        format!(
            "{}#{}{}",
            &body[..self.end],
            Tag::of(self.number, self.content),
            &body[self.end..]
        )
    }
}

impl<'a> Sources<'a> {
    fn new(files: &'a Files, listing: &'a Listing) -> Sources<'a> {
        Sources {
            files,
            folders: &listing.folders,
            excluded: listing
                .excluded
                .iter()
                .map(|folder| files.resolve(folder))
                .collect(),
            located: HashMap::new(),
            read: HashMap::new(),
        }
    }

    fn anchored(&mut self, body: &str, named: &[PathBuf]) -> Option<String> {
        if let Some(row) = numbered(body).or_else(|| bare(body))
            && named.iter().any(|path| self.holds(path, &row))
        {
            return Some(row.tagged(body));
        }
        let folders = self.folders;
        for (prefix, row) in prefixed(body) {
            for folder in folders {
                if let Some(path) = self.locate(prefix, folder)
                    && self.holds(&path, &row)
                {
                    return Some(row.tagged(body));
                }
            }
        }
        None
    }

    fn locate(&mut self, name: &str, folder: &str) -> Option<PathBuf> {
        let joined = Path::new(folder).join(name);
        if let Some(located) = self.located.get(&joined) {
            return located.clone();
        }
        let located = joined
            .to_str()
            .and_then(|joined| self.files.resolve_inside(joined).ok())
            .filter(|path| !self.excluded.iter().any(|folder| path.starts_with(folder)))
            .filter(|path| {
                std::fs::metadata(path)
                    .is_ok_and(|metadata| metadata.is_file() && metadata.len() <= MAX_BYTES)
            });
        self.located.insert(joined, located.clone());
        located
    }

    fn holds(&mut self, path: &Path, row: &Row<'_>) -> bool {
        let files = self.files;
        self.read
            .entry(path.to_owned())
            .or_insert_with(|| {
                let relative_path = files.relative_path(path);
                files
                    .read_text(path, &relative_path)
                    .ok()
                    .map(|(_, text)| text)
            })
            .as_ref()
            .and_then(|text| text.line(row.number))
            .is_some_and(|line| line.strip_suffix('\r').unwrap_or(line) == row.content)
    }
}

fn split_end(line: &str) -> (&str, &str) {
    let body = line.strip_suffix('\n').unwrap_or(line);
    let body = body.strip_suffix('\r').unwrap_or(body);
    (body, &line[body.len()..])
}

fn numbered(body: &str) -> Option<Row<'_>> {
    let start = body.len() - body.trim_start_matches(' ').len();
    let end = start + digits(&body[start..]);
    let number = body[start..end].parse().ok()?;
    let text = body[end..].strip_prefix('\t')?;
    Some(Row::new(number, end, text))
}

fn bare(body: &str) -> Option<Row<'_>> {
    let end = digits(body);
    let number = body[..end].parse().ok()?;
    let text = body[end..]
        .strip_prefix(':')
        .or_else(|| body[end..].strip_prefix('-'))?;
    Some(Row::new(number, end, text))
}

fn prefixed(body: &str) -> impl Iterator<Item = (&str, Row<'_>)> {
    body.char_indices()
        .filter(|&(at, separator)| at > 0 && matches!(separator, ':' | '-'))
        .filter_map(move |(at, separator)| {
            let start = at + 1;
            let end = start + digits(&body[start..]);
            let number = body[start..end].parse().ok()?;
            let text = body[end..].strip_prefix(separator)?;
            Some((&body[..at], Row::new(number, end, text)))
        })
}

fn digits(text: &str) -> usize {
    text.bytes().take_while(u8::is_ascii_digit).count()
}

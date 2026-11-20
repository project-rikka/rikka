use rikka_sync::{Mutex, MutexGuard};
use std::borrow::Cow;
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::{Arc, OnceLock};

use pulldown_cmark::{Event, Parser, Tag};
use rikka_text::LruCache;
use rikka_theme::Theme;
use rikka_ui::text::selection::Region;
use rustc_hash::FxHasher;

use crate::build::{Builder, options};
use crate::rendering::Rendering;

const BUDGET: usize = 4 * 1024 * 1024;

const STREAM_HISTORY: usize = 8;

const PER_LINE: f32 = 72.0;

const FRONT_MATTER: [&str; 2] = ["---", "+++"];

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Block {
    pub(crate) source: Range<usize>,
    pub(crate) text: Range<usize>,
    pub(crate) lines: f32,
    pub(crate) key: u64,
}

#[derive(Debug)]
pub(crate) struct Document {
    source: Arc<str>,
    outline: Outline,
    refs: String,
    refs_key: u64,
    parsed: OnceLock<Parsed>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Outline {
    blocks: Vec<Range<usize>>,
    defs: Vec<Range<usize>>,
    fronted: bool,
}

#[derive(Debug)]
struct Parsed {
    text: Arc<str>,
    blocks: Vec<Block>,
}

impl Document {
    pub(crate) fn parse_cached(source: &Arc<str>) -> Arc<Document> {
        let key = keyed(source);
        let earlier = {
            let mut documents = cache();
            if let Some(found) = documents.lookup(key, source) {
                return found;
            }
            documents.extension_of(source)
        };
        let document = Arc::new(match &earlier {
            Some(earlier) => Document::extend(earlier, source),
            None => Document::parse(source),
        });
        let mut documents = cache();
        if let Some(found) = documents.lookup(key, source) {
            return found;
        }
        documents
            .cache
            .insert(key, Arc::clone(&document), source.len() * 2);
        documents.remember_streaming(earlier.as_ref(), &document);
        document
    }

    fn parse(source: &Arc<str>) -> Document {
        Document::from_outline(source, Outline::parse(source, 0))
    }

    fn extend(earlier: &Document, source: &Arc<str>) -> Document {
        let Some(from) = earlier.reparse_start() else {
            return Document::parse(source);
        };
        let tail = Outline::parse(source, from);
        let defined_before = earlier.outline.defs.iter().all(|def| def.end <= from);
        if tail.fronted || !tail.defs.is_empty() || !defined_before {
            return Document::parse(source);
        }
        let reused = earlier
            .outline
            .blocks
            .iter()
            .take_while(|block| block.start < from)
            .count();
        let mut blocks = earlier.outline.blocks[..reused].to_vec();
        blocks.extend(tail.blocks);
        let document = Document {
            source: Arc::clone(source),
            outline: Outline {
                blocks,
                defs: earlier.outline.defs.clone(),
                fronted: earlier.outline.fronted,
            },
            refs: earlier.refs.clone(),
            refs_key: earlier.refs_key,
            parsed: OnceLock::new(),
        };
        if let Some(previous) = earlier.parsed.get() {
            document
                .parsed
                .get_or_init(|| document.parse_blocks(Some(previous)));
        }
        document
    }

    fn from_outline(source: &Arc<str>, outline: Outline) -> Document {
        let mut refs = String::new();
        for def in &outline.defs {
            if let Some(definition) = source.get(def.clone()) {
                refs.push_str(definition);
                refs.push('\n');
            }
        }
        Document {
            source: Arc::clone(source),
            refs_key: keyed(&refs),
            refs,
            outline,
            parsed: OnceLock::new(),
        }
    }

    fn reparse_start(&self) -> Option<usize> {
        let blocks = &self.outline.blocks;
        let open = blocks.get(blocks.len().checked_sub(2)?)?;
        let fronting = FRONT_MATTER
            .iter()
            .any(|fence| self.source.starts_with(fence));
        if fronting && !self.outline.fronted {
            return None;
        }
        let from = self.source[..open.start]
            .rfind('\n')
            .map_or(0, |newline| newline + 1);
        (from > 0).then_some(from)
    }

    pub(crate) fn refs_key(&self) -> u64 {
        self.refs_key
    }

    pub(crate) fn text(&self) -> &Arc<str> {
        &self.parsed().text
    }

    pub(crate) fn blocks(&self) -> &[Block] {
        &self.parsed().blocks
    }

    pub(crate) fn block_source(&self, block: &Block) -> Cow<'_, str> {
        self.source_text(&block.source)
    }

    fn source_text(&self, source: &Range<usize>) -> Cow<'_, str> {
        let markdown = &self.source[source.clone()];
        if self.refs.is_empty() {
            return Cow::Borrowed(markdown);
        }
        Cow::Owned(format!("{markdown}\n{}", self.refs))
    }

    fn parsed(&self) -> &Parsed {
        self.parsed.get_or_init(|| self.parse_blocks(None))
    }

    fn parse_blocks(&self, earlier: Option<&Parsed>) -> Parsed {
        let ranges = &self.outline.blocks;
        let reused = earlier.map_or(0, |earlier| {
            earlier
                .blocks
                .iter()
                .zip(ranges)
                .take_while(|(block, range)| block.source == **range)
                .count()
        });
        let (mut text, mut blocks) = match earlier.filter(|_| reused > 0) {
            Some(earlier) => (
                earlier.text[..earlier.blocks[reused - 1].text.end].to_owned(),
                earlier.blocks[..reused].to_vec(),
            ),
            None => (String::new(), Vec::with_capacity(ranges.len())),
        };
        for source in &ranges[reused..] {
            let markdown = self.source_text(source);
            let plain_text = plain(&markdown);
            if !text.is_empty() && !plain_text.is_empty() {
                text.push_str(BETWEEN);
            }
            let start = text.len();
            text.push_str(&plain_text);
            let slice = &self.source[source.clone()];
            blocks.push(Block {
                source: source.clone(),
                text: start..text.len(),
                lines: estimated_lines(slice),
                key: keyed(slice),
            });
        }
        Parsed {
            text: Arc::from(text.as_str()),
            blocks,
        }
    }
}

impl Outline {
    fn parse(source: &str, from: usize) -> Outline {
        let at = |range: Range<usize>| range.start + from..range.end + from;
        let mut outline = Outline::default();
        let mut depth = 0usize;
        let mut parser = Parser::new_ext(&source[from..], options()).into_offset_iter();
        for (event, range) in parser.by_ref() {
            match event {
                Event::Start(tag) => {
                    if depth == 0 {
                        if outline.blocks.is_empty() {
                            outline.fronted = matches!(tag, Tag::MetadataBlock(_));
                        }
                        outline.blocks.push(at(range));
                    }
                    depth += 1;
                }
                Event::End(_) => depth = depth.saturating_sub(1),
                _ if depth == 0 => outline.blocks.push(at(range)),
                _ => {}
            }
        }
        outline.defs = parser
            .reference_definitions()
            .iter()
            .map(|(_, def)| at(def.span.clone()))
            .collect();
        outline.defs.sort_by_key(|def| def.start);
        outline
    }
}

fn estimated_lines(markdown: &str) -> f32 {
    let breaks = markdown.lines().count() as f32;
    let wrapped = markdown.len() as f32 / PER_LINE;
    breaks.max(wrapped).max(1.0)
}

pub(crate) const BETWEEN: &str = "\n\n";

thread_local! {
    static PLAIN: Rendering = Rendering {
        region: Some(Region::new(0u8)),
        ..Rendering::from_theme(&Theme::default())
    };
}

fn plain(source: &str) -> Arc<str> {
    PLAIN.with(|rendering| Builder::new(rendering).plain(source))
}

fn keyed(source: &str) -> u64 {
    let mut hasher = FxHasher::default();
    source.len().hash(&mut hasher);
    source.hash(&mut hasher);
    hasher.finish()
}

struct DocumentCache {
    cache: LruCache<u64, Arc<Document>>,
    streaming: VecDeque<Arc<Document>>,
}

impl DocumentCache {
    fn lookup(&mut self, key: u64, source: &str) -> Option<Arc<Document>> {
        self.cache
            .get(key)
            .filter(|found| *found.source == *source)
            .map(Arc::clone)
    }

    fn extension_of(&self, source: &str) -> Option<Arc<Document>> {
        self.streaming
            .iter()
            .filter(|earlier| {
                earlier.source.len() < source.len() && source.starts_with(&*earlier.source)
            })
            .max_by_key(|earlier| earlier.source.len())
            .cloned()
    }

    fn remember_streaming(&mut self, earlier: Option<&Arc<Document>>, document: &Arc<Document>) {
        if let Some(earlier) = earlier {
            self.streaming
                .retain(|document| !Arc::ptr_eq(document, earlier));
        }
        if self.streaming.len() == STREAM_HISTORY {
            self.streaming.pop_front();
        }
        self.streaming.push_back(Arc::clone(document));
    }
}

fn cache() -> MutexGuard<'static, DocumentCache> {
    static CACHE: OnceLock<Mutex<DocumentCache>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            Mutex::new(DocumentCache {
                cache: LruCache::new(BUDGET),
                streaming: VecDeque::with_capacity(STREAM_HISTORY),
            })
        })
        .lock()
}

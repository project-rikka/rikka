use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use rikka_text::LruCache;
use rikka_text_document::Rope;
use rustc_hash::FxHasher;
use tree_sitter as ts;

use crate::injection::InjectionLayers;
use crate::language::Language;
use crate::region::Region;
use crate::rope::chunk_at;

const CACHE_BUDGET: usize = 4 * 1024 * 1024;

const MAX_SNIPPET_LEN: usize = 256 * 1024;

thread_local! {
    static SNIPPETS: RefCell<LruCache<SnippetKey, CachedSnippet>> = RefCell::new(LruCache::new(CACHE_BUDGET));
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct SnippetKey {
    language: Language,
    length: usize,
    digest: u64,
}

struct CachedSnippet {
    text: Arc<str>,
    regions: Arc<[Region]>,
}

pub(crate) fn regions_of(language: Language, text: &str) -> Arc<[Region]> {
    if text.len() > MAX_SNIPPET_LEN {
        return Arc::default();
    }
    let key = SnippetKey {
        language,
        length: text.len(),
        digest: digest_of(text),
    };
    let cached = SNIPPETS.with_borrow_mut(|cache| {
        let snippet = cache.get(key)?;
        (&*snippet.text == text).then(|| Arc::clone(&snippet.regions))
    });
    if let Some(cached) = cached {
        return cached;
    }

    let regions: Arc<[Region]> = read(language, text).into();
    let snippet = CachedSnippet {
        text: Arc::from(text),
        regions: Arc::clone(&regions),
    };
    let bytes = text.len() + std::mem::size_of_val(&*regions);
    SNIPPETS.with_borrow_mut(|cache| cache.insert(key, snippet, bytes));
    regions
}

fn read(language: Language, text: &str) -> Vec<Region> {
    let mut regions = Vec::new();
    let Some(rules) = language.rules() else {
        return regions;
    };
    let mut parser = ts::Parser::new();
    if parser.set_language(&rules.language).is_err() {
        return regions;
    }
    let rope = Rope::from(text);
    let Some(tree) = parser.parse_with_options(&mut |byte, _| chunk_at(&rope, byte), None, None)
    else {
        return regions;
    };
    crate::read(
        rules,
        tree.root_node(),
        &rope,
        0..rope.len(),
        &mut InjectionLayers::default(),
        &mut regions,
    );
    regions
}

fn digest_of(text: &str) -> u64 {
    let mut hasher = FxHasher::default();
    text.hash(&mut hasher);
    hasher.finish()
}

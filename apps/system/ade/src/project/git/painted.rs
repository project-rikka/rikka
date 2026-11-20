use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use ade_git::Diff;
use rikka_app::prelude::*;
use rikka_chat::Ink;
use rikka_syntax::Language;
use rikka_theme::{Syntax, Token};

use crate::project::git::diff::inked;

const LONGEST: usize = 256 * 1024;
#[derive(Clone, Copy, Debug)]
pub(in crate::project) struct Colours {
    syntax: Syntax,
    stamp: u64,
}

impl Colours {
    pub(in crate::project) fn for_syntax(syntax: Syntax) -> Colours {
        let mut hasher = DefaultHasher::new();
        for token in Token::ALL {
            let color = syntax.color(token);
            color.r.to_bits().hash(&mut hasher);
            color.g.to_bits().hash(&mut hasher);
            color.b.to_bits().hash(&mut hasher);
            color.a.to_bits().hash(&mut hasher);
        }
        Colours {
            syntax,
            stamp: hasher.finish(),
        }
    }
}

#[derive(Debug, Default)]
pub(in crate::project) struct Painted {
    cache: RefCell<HashMap<usize, CachedSpans>>,
}

#[derive(Debug)]
struct CachedSpans {
    stamp: u64,
    spans: Arc<[Arc<[TextSpan]>]>,
}

impl Painted {
    pub(in crate::project) fn spans(
        &self,
        file: usize,
        diff: &Diff,
        colours: Colours,
    ) -> Arc<[Arc<[TextSpan]>]> {
        let mut cache = self.cache.borrow_mut();
        if let Some(cached) = cache
            .get(&file)
            .filter(|cached| cached.stamp == colours.stamp)
        {
            return Arc::clone(&cached.spans);
        }
        let spans = paint(diff, colours.syntax);
        cache.insert(
            file,
            CachedSpans {
                stamp: colours.stamp,
                spans: Arc::clone(&spans),
            },
        );
        spans
    }
}

fn paint(diff: &Diff, colors: Syntax) -> Arc<[Arc<[TextSpan]>]> {
    let inked: Vec<(Ink, &str)> = diff.lines().iter().map(inked).collect();
    let content: usize = inked.iter().map(|(_, line)| line.len()).sum();
    if content > LONGEST {
        return Arc::default();
    }
    rikka_chat::highlighted_code(Language::from_path(diff.path()), colors, &inked).into()
}

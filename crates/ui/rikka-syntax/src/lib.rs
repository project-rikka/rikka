mod comment;
mod grammar;
mod injection;
mod language;
mod lexicon;
mod parse;
mod queries;
mod region;
mod rope;
mod snippet;
mod suggestion;
mod token;
mod words;

use std::ops::Range;

use rikka_text::TextSpan;
use rikka_text_document::Rope;
use rikka_theme::Syntax;
use tree_sitter as ts;

use crate::grammar::Rules;
use crate::injection::InjectionLayers;
use crate::rope::RopeProvider;

pub use comment::Comment;
pub use language::Language;
pub use lexicon::Lexicon;
pub use parse::Parsed;
pub use suggestion::{Kind, Suggestion};
pub use words::{QUOTES, is_word};

use crate::region::Region;
use crate::snippet::regions_of;

fn read(
    rules: &'static Rules,
    root: ts::Node<'_>,
    rope: &Rope,
    within: Range<usize>,
    injected: &mut InjectionLayers,
    into: &mut Vec<Region>,
) {
    let within = within.start.min(rope.len())..within.end.min(rope.len());
    let mut captured = Vec::new();
    region::gather(rules, root, RopeProvider(rope), &within, 0, &mut captured);
    injected.gather(rules, root, rope, &within, &mut captured);
    region::flatten(captured, within, into);
}

fn spans_of(regions: &[Region], colors: Syntax) -> Vec<TextSpan> {
    regions
        .iter()
        .map(|region| TextSpan::new(region.range.clone()).color(colors.color(region.token)))
        .collect()
}

use std::borrow::Cow;
use std::collections::VecDeque;
use std::ops::Range;

use rikka_text_document::Rope;
use rikka_text_document::Splice;
use tree_sitter as ts;
use tree_sitter::StreamingIterator;

use crate::grammar::Rules;
use crate::language::Language;
use crate::region::{self, RawRegion};
use crate::rope::{RopeProvider, chunk_at, edit_of, span_of};

const MAX_DEPTH: u8 = 3;

const CACHED_LAYERS: usize = 16;

const MAX_INJECTED_BYTES: usize = 128 * 1024;

struct Layer {
    language: Language,
    ranges: Vec<Range<usize>>,
    tree: ts::Tree,
}

#[derive(Default)]
pub(crate) struct InjectionLayers {
    parser: ts::Parser,
    layers: VecDeque<Layer>,
}

struct InjectionRequest {
    language: Language,
    ranges: Vec<Range<usize>>,
}

impl InjectionLayers {
    pub(crate) fn clear(&mut self) {
        self.layers.clear();
    }
    pub(crate) fn apply_splice(&mut self, splice: &Splice) {
        let touched = splice.start..splice.old_end.max(splice.start);
        self.layers.retain(|layer| {
            layer
                .ranges
                .iter()
                .all(|range| range.end <= touched.start || range.start >= touched.end)
        });
        let edit = edit_of(splice);
        let shift = splice.new_end as isize - splice.old_end as isize;
        for layer in &mut self.layers {
            for range in &mut layer.ranges {
                if range.start >= splice.old_end {
                    range.start = range.start.saturating_add_signed(shift);
                    range.end = range.end.saturating_add_signed(shift);
                }
            }
            layer.tree.edit(&edit);
        }
    }

    pub(crate) fn gather(
        &mut self,
        host: &'static Rules,
        root: ts::Node<'_>,
        rope: &Rope,
        within: &Range<usize>,
        into: &mut Vec<RawRegion>,
    ) {
        let mut requests = Vec::new();
        seek(host, root, rope, within, &mut requests);
        for depth in 1..=MAX_DEPTH {
            let mut deeper = Vec::new();
            for request in requests.drain(..) {
                let Some(rules) = request.language.rules() else {
                    continue;
                };
                let Some(tree) = self.layer(request, rules, rope) else {
                    continue;
                };
                region::gather(
                    rules,
                    tree.root_node(),
                    RopeProvider(rope),
                    within,
                    depth,
                    into,
                );
                if depth < MAX_DEPTH {
                    seek(rules, tree.root_node(), rope, within, &mut deeper);
                }
            }
            requests = deeper;
            if requests.is_empty() {
                break;
            }
        }
    }

    fn layer(
        &mut self,
        request: InjectionRequest,
        rules: &'static Rules,
        rope: &Rope,
    ) -> Option<ts::Tree> {
        if let Some(layer) = self
            .layers
            .iter()
            .find(|layer| layer.language == request.language && layer.ranges == request.ranges)
        {
            return Some(layer.tree.clone());
        }
        self.parser.set_language(&rules.language).ok()?;
        let spans: Vec<ts::Range> = request
            .ranges
            .iter()
            .map(|range| span_of(rope, range))
            .collect();
        self.parser.set_included_ranges(&spans).ok()?;
        let tree =
            self.parser
                .parse_with_options(&mut |byte, _| chunk_at(rope, byte), None, None)?;
        if self.layers.len() == CACHED_LAYERS {
            self.layers.pop_front();
        }
        self.layers.push_back(Layer {
            language: request.language,
            ranges: request.ranges,
            tree: tree.clone(),
        });
        Some(tree)
    }
}

fn seek(
    rules: &'static Rules,
    node: ts::Node<'_>,
    rope: &Rope,
    within: &Range<usize>,
    into: &mut Vec<InjectionRequest>,
) {
    let Some(injections) = &rules.injections else {
        return;
    };
    let mut cursor = ts::QueryCursor::new();
    cursor.set_byte_range(within.clone());
    let mut matches = cursor.matches(&injections.query, node, RopeProvider(rope));
    let mut combined: Vec<(usize, Language, Vec<Range<usize>>)> = Vec::new();
    while let Some(query_match) = matches.next() {
        let mut named: Option<Cow<'static, str>> = None;
        let mut content = Vec::new();
        for capture in query_match.captures() {
            if Some(capture.index) == injections.language {
                named = Some(Cow::Owned(rope.slice(capture.node.byte_range())));
            } else if capture.index == injections.content {
                content.push(capture.node);
            }
        }
        let mut children = false;
        let mut combined_injection = false;
        for property in injections
            .query
            .property_settings(query_match.pattern_index)
        {
            match property.key.as_ref() {
                "injection.language" if named.is_none() => {
                    named = property.value.as_deref().map(Cow::Borrowed);
                }
                "injection.self" if named.is_none() => named = Some(Cow::Borrowed(rules.name)),
                "injection.include-children" => children = true,
                "injection.combined" => combined_injection = true,
                _ => {}
            }
        }
        let Some(language) = named.as_deref().and_then(Language::named) else {
            continue;
        };
        let ranges = content_ranges(&content, children);
        if ranges.is_empty() {
            continue;
        }
        match combined_injection {
            true => match combined.iter_mut().find(|(pattern, joined, _)| {
                *pattern == query_match.pattern_index && *joined == language
            }) {
                Some((_, _, joined)) => joined.extend(ranges),
                None => combined.push((query_match.pattern_index, language, ranges)),
            },
            false => push_request(into, language, ranges),
        }
    }
    for (_, language, ranges) in combined {
        push_request(into, language, ranges);
    }
}

fn push_request(into: &mut Vec<InjectionRequest>, language: Language, ranges: Vec<Range<usize>>) {
    let measured: usize = ranges.iter().map(|range| range.end - range.start).sum();
    if measured > MAX_INJECTED_BYTES {
        return;
    }
    into.push(InjectionRequest { language, ranges });
}

fn content_ranges(content: &[ts::Node<'_>], children: bool) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    for node in content {
        if children {
            ranges.push(node.byte_range());
            continue;
        }
        let mut cursor = node.walk();
        let mut at = node.start_byte();
        for child in node.named_children(&mut cursor) {
            if child.start_byte() > at {
                ranges.push(at..child.start_byte());
            }
            at = at.max(child.end_byte());
        }
        if at < node.end_byte() {
            ranges.push(at..node.end_byte());
        }
    }
    ranges.retain(|range| range.start < range.end);
    ranges
}

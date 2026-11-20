use crate::parse::media::{Media, QueryList};
use crate::parse::selectors::{Scope, Selector, Work};

const MAX_MEDIA_NESTING: usize = 8;

#[derive(Clone, Debug)]
struct Rule {
    selector: Selector,
    declarations: usize,
    media: Vec<usize>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Sheet {
    rules: Vec<Rule>,
    blocks: Vec<Vec<(String, String)>>,
    queries: Vec<QueryList>,
}

impl Sheet {
    pub(crate) fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    pub(crate) fn queries(&self) -> &[QueryList] {
        &self.queries
    }

    pub(crate) fn parse(&mut self, css: &str) {
        let css = without_comments(css);
        let mut conditions = Vec::new();
        self.rules_in(&css, &mut conditions);
    }

    fn rules_in(&mut self, css: &str, conditions: &mut Vec<usize>) {
        let mut rest = css;
        while let Some(open) = rest.find('{') {
            let prelude = rest[..open].trim();
            let Some(close) = block_end(&rest[open..]) else {
                return;
            };
            let body = &rest[open + 1..open + close];
            rest = &rest[open + close + 1..];
            let prelude = prelude.rsplit(';').next().unwrap_or(prelude).trim();
            if let Some(at_rule) = prelude.strip_prefix('@') {
                let (name, condition) = at_rule
                    .split_once(char::is_whitespace)
                    .unwrap_or((at_rule, ""));
                if name.eq_ignore_ascii_case("media") && conditions.len() < MAX_MEDIA_NESTING {
                    conditions.push(self.queries.len());
                    self.queries.push(QueryList::parse(condition));
                    self.rules_in(body, conditions);
                    conditions.pop();
                }
                continue;
            }
            let declarations = declarations(body);
            if declarations.is_empty() {
                continue;
            }
            let block = self.blocks.len();
            self.blocks.push(declarations);
            for selector in split_selectors(prelude) {
                if let Some(selector) = Selector::parse(selector) {
                    self.rules.push(Rule {
                        selector,
                        declarations: block,
                        media: conditions.clone(),
                    });
                }
            }
        }
    }

    pub(crate) fn active(&self, media: &Media) -> Vec<bool> {
        self.queries
            .iter()
            .map(|query| query.matches(media))
            .collect()
    }

    pub(crate) fn matching<S: Scope>(
        &self,
        scope: &S,
        node: S::Node,
        active: &[bool],
        work: &Work,
    ) -> Vec<(String, String)> {
        let mut matched: Vec<&Rule> = self
            .rules
            .iter()
            .filter(|rule| rule.media.iter().all(|query| active[*query]))
            .filter(|rule| rule.selector.matches(scope, node, work))
            .collect();
        matched.sort_by_key(|rule| rule.selector.specificity());
        matched
            .into_iter()
            .flat_map(|rule| self.blocks[rule.declarations].iter().cloned())
            .collect()
    }
}

fn split_selectors(prelude: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let (mut depth, mut start, mut quote) = (0usize, 0, None);
    for (at, character) in prelude.char_indices() {
        match (quote, character) {
            (Some(open), _) if open == character => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(character),
            (None, '(' | '[') => depth += 1,
            (None, ')' | ']') => depth = depth.saturating_sub(1),
            (None, ',') if depth == 0 => {
                parts.push(&prelude[start..at]);
                start = at + 1;
            }
            _ => {}
        }
    }
    parts.push(&prelude[start..]);
    parts
}

fn without_comments(css: &str) -> String {
    let mut clean = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        clean.push_str(&rest[..start]);
        rest = match rest[start + 2..].find("*/") {
            Some(end) => &rest[start + 2 + end + 2..],
            None => "",
        };
    }
    clean.push_str(rest);
    clean.replace("<!--", "").replace("-->", "")
}

fn block_end(text: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (at, byte) in text.bytes().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(at);
                }
            }
            _ => {}
        }
    }
    None
}

pub(crate) fn declarations(text: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    let mut push = |piece: &str| {
        if let Some((name, value)) = piece.split_once(':') {
            let value = value.trim();
            let value = value
                .strip_suffix("!important")
                .map_or(value, str::trim_end);
            let name = name.trim();
            if !name.is_empty() && !value.is_empty() {
                let name = match name.starts_with("--") {
                    true => name.to_owned(),
                    false => name.to_ascii_lowercase(),
                };
                found.push((name, value.to_owned()));
            }
        }
    };
    for (at, byte) in text.bytes().enumerate() {
        match byte {
            b'(' => depth += 1,
            b')' => depth = depth.saturating_sub(1),
            b';' if depth == 0 => {
                push(&text[start..at]);
                start = at + 1;
            }
            _ => {}
        }
    }
    push(&text[start..]);
    found
}

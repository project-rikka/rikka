use std::collections::HashMap;

use crate::mime_type::MimeType;

const DEFAULT_WEIGHT: u32 = 50;

#[derive(Clone, Debug)]
struct Pattern {
    glob: String,
    mime: MimeType,
    weight: u32,
    case_sensitive: bool,
}

#[derive(Debug, Default)]
pub(crate) struct Globs {
    literals: HashMap<String, Vec<Pattern>>,
    suffixes: HashMap<String, Vec<Pattern>>,
    others: Vec<Pattern>,
}

impl Globs {
    pub(crate) fn extend_from(&mut self, text: &str) {
        for line in text.lines() {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let Some(pattern) = parse_line(line) else {
                continue;
            };
            if let Some(literal) = as_literal(&pattern.glob) {
                self.literals.entry(literal).or_default().push(pattern);
            } else if let Some(suffix) = as_suffix(&pattern.glob) {
                self.suffixes.entry(suffix).or_default().push(pattern);
            } else {
                self.others.push(pattern);
            }
        }
    }

    pub(crate) fn patterns(&self) -> impl Iterator<Item = (&str, &MimeType)> {
        self.literals
            .values()
            .chain(self.suffixes.values())
            .flatten()
            .chain(&self.others)
            .map(|pattern| (pattern.glob.as_str(), &pattern.mime))
    }

    pub(crate) fn lookup(&self, name: &str) -> Vec<MimeType> {
        let lowered = name.to_ascii_lowercase();

        if let Some(hits) = best(self.literals.get(name), name, &lowered) {
            return hits;
        }
        if let Some(hits) = best(self.literals.get(&lowered), name, &lowered) {
            return hits;
        }
        let mut matched: Option<Vec<MimeType>> = None;
        let mut longest = 0;
        for (index, _) in lowered
            .char_indices()
            .filter(|&(index, c)| c == '.' && index > 0)
        {
            let suffix = &lowered[index..];
            if suffix.len() <= longest {
                continue;
            }
            if let Some(hits) = best(self.suffixes.get(suffix), name, &lowered) {
                longest = suffix.len();
                matched = Some(hits);
            }
        }
        if let Some(hits) = matched {
            return hits;
        }
        best(Some(&self.others), name, &lowered).unwrap_or_default()
    }
}

fn best(bucket: Option<&Vec<Pattern>>, name: &str, lowered: &str) -> Option<Vec<MimeType>> {
    let bucket = bucket?;
    let mut hits: Vec<&Pattern> = bucket
        .iter()
        .filter(|pattern| matches(pattern, name, lowered))
        .collect();
    if hits.is_empty() {
        return None;
    }
    let top = hits.iter().map(|pattern| pattern.weight).max().unwrap_or(0);
    hits.retain(|pattern| pattern.weight == top);
    let mut types: Vec<MimeType> = hits
        .into_iter()
        .map(|pattern| pattern.mime.clone())
        .collect();
    types.dedup();
    Some(types)
}

fn matches(pattern: &Pattern, name: &str, lowered: &str) -> bool {
    let subject = if pattern.case_sensitive {
        name
    } else {
        lowered
    };
    crate::pattern::matches_glob(&pattern.glob, subject)
}

fn parse_line(line: &str) -> Option<Pattern> {
    let mut fields = line.splitn(4, ':');
    let weight = fields.next()?.parse().unwrap_or(DEFAULT_WEIGHT);
    let mime = MimeType::parse(fields.next()?)?;
    let glob = fields.next()?;
    let case_sensitive = fields
        .next()
        .is_some_and(|flags| flags.split(',').any(|flag| flag == "cs"));
    Some(Pattern {
        glob: if case_sensitive {
            glob.to_owned()
        } else {
            glob.to_ascii_lowercase()
        },
        mime,
        weight,
        case_sensitive,
    })
}

fn as_literal(glob: &str) -> Option<String> {
    (!glob.contains(['*', '?', '['])).then(|| glob.to_owned())
}

fn as_suffix(glob: &str) -> Option<String> {
    let rest = glob.strip_prefix('*')?;
    (rest.starts_with('.') && !rest.contains(['*', '?', '['])).then(|| rest.to_ascii_lowercase())
}

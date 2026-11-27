use std::cmp::Reverse;
use std::collections::HashMap;
use std::ops::Range;

const MIN_SIMILARITY: f64 = 0.5;

type Bigrams = HashMap<(char, char), usize>;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Closest {
    pub(crate) lines: Range<usize>,
    pub(crate) spacing_only: bool,
}

pub(crate) fn occurrences(content: &str, needle: &str) -> Vec<usize> {
    content.match_indices(needle).map(|(at, _)| at).collect()
}

pub(crate) fn overlapping(content: &str, needle: &str) -> Vec<usize> {
    let step = needle.chars().next().map_or(1, char::len_utf8);
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(at) = content[from..].find(needle) {
        found.push(from + at);
        from += at + step;
    }
    found
}

pub(crate) fn line_of(content: &str, at: usize) -> usize {
    content[..at].bytes().filter(|&byte| byte == b'\n').count()
}

pub(crate) fn closest(lines: &[&str], needle: &str) -> Option<Closest> {
    let wanted: Vec<String> = needle
        .trim_end_matches('\n')
        .split('\n')
        .map(squeeze)
        .collect();
    let keys: Vec<String> = lines.iter().map(|line| squeeze(line)).collect();
    match wanted.as_slice() {
        [single] => closest_line(&keys, single),
        _ => closest_block(&keys, &wanted),
    }
}

pub(crate) fn unnumbered(needle: &str) -> Option<String> {
    let lines: Vec<&str> = needle.trim_end_matches('\n').split('\n').collect();
    let stripped: Vec<&str> = lines
        .iter()
        .map(|line| {
            let digits = line.trim_start_matches(' ');
            let rest = digits.trim_start_matches(|c: char| c.is_ascii_digit());
            let numbered = rest.len() < digits.len();
            let unnumbered = rest.strip_prefix(['\t', ':', '-'])?;
            numbered.then_some(unnumbered)
        })
        .collect::<Option<Vec<&str>>>()?;
    let mut unnumbered = stripped.join("\n");
    if needle.ends_with('\n') {
        unnumbered.push('\n');
    }
    Some(unnumbered)
}

fn closest_line(keys: &[String], wanted: &str) -> Option<Closest> {
    if !meaningful(wanted) {
        return None;
    }
    if let Some(at) = keys.iter().position(|key| key.contains(wanted)) {
        return Some(Closest {
            lines: at..at + 1,
            spacing_only: true,
        });
    }
    let pairs = bigrams(wanted);
    let (at, similarity) = keys
        .iter()
        .enumerate()
        .map(|(at, key)| (at, dice(&pairs, &bigrams(key))))
        .max_by(|one, other| one.1.total_cmp(&other.1).then(other.0.cmp(&one.0)))?;
    (similarity >= MIN_SIMILARITY).then_some(Closest {
        lines: at..at + 1,
        spacing_only: false,
    })
}

fn closest_block(keys: &[String], wanted: &[String]) -> Option<Closest> {
    let mut anchors: HashMap<&str, Vec<usize>> = HashMap::new();
    for (index, key) in wanted.iter().enumerate() {
        if meaningful(key) {
            anchors.entry(key.as_str()).or_default().push(index);
        }
    }
    let anchored: usize = anchors.values().map(Vec::len).sum();
    if anchored == 0 {
        return None;
    }

    let mut votes: HashMap<usize, usize> = HashMap::new();
    for (at, key) in keys.iter().enumerate() {
        for &index in anchors.get(key.as_str()).into_iter().flatten() {
            if let Some(start) = at.checked_sub(index) {
                *votes.entry(start).or_default() += 1;
            }
        }
    }
    let (start, count) = votes
        .into_iter()
        .max_by_key(|&(start, count)| (count, Reverse(start)))?;
    if count * 2 < anchored {
        return None;
    }

    let lines = start..(start + wanted.len()).min(keys.len());
    let spacing_only = keys[lines.clone()] == *wanted;
    Some(Closest {
        lines,
        spacing_only,
    })
}

fn squeeze(line: &str) -> String {
    line.split_whitespace().collect::<Vec<&str>>().join(" ")
}

fn meaningful(text: &str) -> bool {
    text.chars().any(char::is_alphanumeric)
}

fn dice(one: &Bigrams, other: &Bigrams) -> f64 {
    let total: usize = one.values().sum::<usize>() + other.values().sum::<usize>();
    if total == 0 {
        return 0.0;
    }
    let shared: usize = one
        .iter()
        .map(|(pair, count)| (*count).min(other.get(pair).copied().unwrap_or(0)))
        .sum();
    (2 * shared) as f64 / total as f64
}

fn bigrams(text: &str) -> Bigrams {
    let chars: Vec<char> = text.chars().collect();
    let mut pairs = HashMap::new();
    for pair in chars.windows(2) {
        *pairs.entry((pair[0], pair[1])).or_default() += 1;
    }
    pairs
}

use std::collections::HashMap;

use crate::faces::Morph;
use crate::limb::Side;
use crate::names::normalized;

const SEPARATORS: [char; 4] = ['_', '.', ' ', '-'];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SidedMorph<'a> {
    pub name: &'a str,
    pub whole: Option<&'a Morph>,
    pub left: Option<&'a Morph>,
    pub right: Option<&'a Morph>,
}

impl<'a> SidedMorph<'a> {
    fn alone(morph: &'a Morph) -> Self {
        Self {
            name: morph.name(),
            whole: Some(morph),
            left: None,
            right: None,
        }
    }

    #[must_use]
    pub fn has_sides(&self) -> bool {
        self.left.is_some() || self.right.is_some()
    }

    pub fn members(&self) -> impl Iterator<Item = &'a Morph> {
        self.whole.into_iter().chain(self.left).chain(self.right)
    }

    fn side_mut(&mut self, side: Side) -> &mut Option<&'a Morph> {
        match side {
            Side::Left => &mut self.left,
            Side::Right => &mut self.right,
        }
    }
}

#[must_use]
pub fn group_sides<'a>(morphs: impl IntoIterator<Item = &'a Morph>) -> Vec<SidedMorph<'a>> {
    let morphs: Vec<&Morph> = morphs.into_iter().collect();
    let marked: Vec<Option<(&str, Side)>> =
        morphs.iter().map(|morph| marked(morph.name())).collect();
    let wholes: HashMap<String, usize> = morphs
        .iter()
        .zip(&marked)
        .enumerate()
        .filter(|(_, (_, marked))| marked.is_none())
        .map(|(at, (morph, _))| (normalized(morph.name()), at))
        .rev()
        .collect();
    let mut pairs: HashMap<String, [Option<usize>; 2]> = HashMap::new();
    for (at, marked) in marked.iter().enumerate() {
        if let Some((base, side)) = marked {
            let slot = &mut pairs.entry(normalized(base)).or_default()[*side as usize];
            slot.get_or_insert(at);
        }
    }
    let mut grouped: Vec<Option<SidedMorph<'a>>> = morphs
        .iter()
        .map(|morph| Some(SidedMorph::alone(morph)))
        .collect();
    for (base, variants) in pairs {
        let whole = wholes.get(&base).copied();
        let present = variants.iter().flatten().count() + usize::from(whole.is_some());
        if present < 2 {
            continue;
        }
        let first = whole
            .into_iter()
            .chain(variants.iter().flatten().copied())
            .min()
            .unwrap_or_default();
        let mut sided = SidedMorph {
            name: match whole {
                Some(at) => morphs[at].name(),
                None => variants
                    .iter()
                    .flatten()
                    .find_map(|at| marked[*at].map(|(base, _)| base))
                    .unwrap_or_default(),
            },
            whole: whole.map(|at| morphs[at]),
            left: None,
            right: None,
        };
        for (side, at) in Side::BOTH.into_iter().zip(variants) {
            let Some(at) = at else {
                continue;
            };
            *sided.side_mut(side) = Some(morphs[at]);
            if at != first {
                grouped[at] = None;
            }
        }
        if let Some(at) = whole.filter(|at| *at != first) {
            grouped[at] = None;
        }
        grouped[first] = Some(sided);
    }
    grouped.into_iter().flatten().collect()
}

fn marked(name: &str) -> Option<(&str, Side)> {
    let side_of = |letter: char| match letter {
        '左' => Some(Side::Left),
        '右' => Some(Side::Right),
        _ => None,
    };
    let last = name.chars().next_back()?;
    if let Some(side) = side_of(last) {
        let base = &name[..name.len() - last.len_utf8()];
        return (!base.is_empty()).then_some((base, side));
    }
    let first = name.chars().next()?;
    if let Some(side) = side_of(first) {
        let base = &name[first.len_utf8()..];
        return (!base.is_empty()).then_some((base, side));
    }
    latin(name)
}

fn latin(name: &str) -> Option<(&str, Side)> {
    let words = [
        ("left", Side::Left),
        ("right", Side::Right),
        ("l", Side::Left),
        ("r", Side::Right),
    ];
    for (word, side) in words {
        if name.len() <= word.len() || !name.is_char_boundary(name.len() - word.len()) {
            continue;
        }
        let (base, end) = name.split_at(name.len() - word.len());
        if !end.eq_ignore_ascii_case(word) {
            continue;
        }
        if let Some(stripped) = base
            .strip_suffix(SEPARATORS)
            .filter(|base| !base.is_empty())
        {
            return Some((stripped, side));
        }
        let camel = word.len() > 1
            && end.starts_with(|letter: char| letter.is_ascii_uppercase())
            && base.ends_with(|letter: char| letter.is_lowercase());
        if camel {
            return Some((base, side));
        }
    }
    None
}

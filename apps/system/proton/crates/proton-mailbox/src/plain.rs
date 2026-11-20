use std::ops::Range;
use std::sync::Arc;

const SCHEMES: [&str; 3] = ["https://", "http://", "mailto:"];
const TRAILING: &[char] = &['.', ',', ':', ';', '!', '?', '\'', '"', '*'];
const PAIRS: [(char, char); 2] = [('(', ')'), ('[', ']')];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plain {
    text: Arc<str>,
    links: Arc<[Range<usize>]>,
}

impl Plain {
    #[must_use]
    pub fn new(text: impl Into<Arc<str>>) -> Plain {
        let text = text.into();
        let links = links(&text).into();
        Plain { text, links }
    }

    #[must_use]
    pub fn text(&self) -> &Arc<str> {
        &self.text
    }

    #[must_use]
    pub fn links(&self) -> &Arc<[Range<usize>]> {
        &self.links
    }

    #[must_use]
    pub fn link(&self, index: usize) -> Option<&str> {
        self.links.get(index).map(|range| &self.text[range.clone()])
    }
}

fn links(text: &str) -> Vec<Range<usize>> {
    let lowered = text.to_ascii_lowercase();
    let mut next = SCHEMES.map(|scheme| lowered.find(scheme));
    let mut found = Vec::new();
    let mut at = 0;
    loop {
        for (slot, scheme) in next.iter_mut().zip(SCHEMES) {
            if slot.is_some_and(|start| start < at) {
                *slot = lowered[at..].find(scheme).map(|offset| at + offset);
            }
        }
        let Some((start, length)) = next
            .iter()
            .zip(SCHEMES)
            .filter_map(|(slot, scheme)| Some(((*slot)?, scheme.len())))
            .min()
        else {
            return found;
        };
        let standalone = text[..start]
            .chars()
            .next_back()
            .is_none_or(|before| !before.is_ascii_alphanumeric());
        let end = match standalone {
            true => start + extent(&text[start..]),
            false => start,
        };
        if end > start + length {
            found.push(start..end);
            at = end;
        } else {
            at = start + length;
        }
    }
}

fn extent(link: &str) -> usize {
    let mut end = link
        .find(|character: char| {
            !character.is_ascii_graphic() || matches!(character, '<' | '>' | '"' | '`' | '\\')
        })
        .unwrap_or(link.len());
    let opened = PAIRS.map(|(open, _)| link[..end].matches(open).count());
    let mut closed = PAIRS.map(|(_, close)| link[..end].matches(close).count());
    while let Some(last) = link[..end].chars().next_back() {
        if let Some(pair) = PAIRS.iter().position(|&(_, close)| close == last) {
            if closed[pair] <= opened[pair] {
                break;
            }
            closed[pair] -= 1;
        } else if !TRAILING.contains(&last) {
            break;
        }
        end -= 1;
    }
    end
}

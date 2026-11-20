use crate::mime_type::MimeType;

const SIGNATURE: &[u8] = b"MIME-Magic\0\n";

pub(crate) const SNIFF_BYTES: usize = 64 * 1024;

const CONTENT_BEATS_NAME: u32 = 80;

#[derive(Debug)]
struct Rule {
    offset: usize,
    range: usize,
    value: Vec<u8>,
    mask: Option<Vec<u8>>,
    children: Vec<Rule>,
}

impl Rule {
    fn matches(&self, data: &[u8]) -> bool {
        if !self.matches_here(data) {
            return false;
        }
        self.children.is_empty() || self.children.iter().any(|child| child.matches(data))
    }

    fn matches_here(&self, data: &[u8]) -> bool {
        let last = data.len().saturating_sub(self.value.len());
        let end = (self.offset + self.range).min(last + 1);
        (self.offset..end).any(|start| {
            let Some(window) = data.get(start..start + self.value.len()) else {
                return false;
            };
            match &self.mask {
                None => window == self.value,
                Some(mask) => window
                    .iter()
                    .zip(&self.value)
                    .zip(mask)
                    .all(|((&byte, &value), &mask)| byte & mask == value & mask),
            }
        })
    }
}

#[derive(Debug)]
struct Section {
    priority: u32,
    mime: MimeType,
    roots: Vec<Rule>,
}

#[derive(Debug, Default)]
pub(crate) struct Magic {
    sections: Vec<Section>,
}

impl Magic {
    pub(crate) fn extend_from(&mut self, bytes: &[u8]) {
        let Some(mut rest) = bytes.strip_prefix(SIGNATURE) else {
            return;
        };
        while let Some((section, tail)) = read_section(rest) {
            self.sections.push(section);
            rest = tail;
        }
        self.sections
            .sort_by_key(|section| std::cmp::Reverse(section.priority));
    }

    pub(crate) fn lookup(
        &self,
        data: &[u8],
        candidates: &[MimeType],
        resolve: impl Fn(&MimeType) -> MimeType,
    ) -> Option<MimeType> {
        let mut overruling = None;
        let mut best = None;
        for section in &self.sections {
            if !section.roots.iter().any(|rule| rule.matches(data)) {
                continue;
            }
            let mime = resolve(&section.mime);
            if candidates.contains(&mime) {
                return Some(mime);
            }
            if overruling.is_none() && section.priority >= CONTENT_BEATS_NAME {
                overruling = Some(mime.clone());
            }
            if best.is_none() {
                best = Some(mime);
            }
        }
        if candidates.is_empty() {
            return overruling.or(best);
        }
        overruling
    }
}

fn read_section(bytes: &[u8]) -> Option<(Section, &[u8])> {
    let rest = bytes.strip_prefix(b"[")?;
    let close = rest.iter().position(|&byte| byte == b']')?;
    let (header, rest) = rest.split_at(close);
    let rest = rest.get(1..)?.strip_prefix(b"\n")?;

    let header = std::str::from_utf8(header).ok()?;
    let (priority, mime) = header.split_once(':')?;
    let section = Section {
        priority: priority.parse().ok()?,
        mime: MimeType::parse(mime)?,
        roots: Vec::new(),
    };

    let mut section = section;
    let mut rest = rest;
    let mut path: Vec<usize> = Vec::new();
    while !rest.starts_with(b"[") && !rest.is_empty() {
        let Some((indent, rule, tail)) = read_rule(rest) else {
            break;
        };
        rest = tail;
        path.truncate(indent);
        let mut siblings = &mut section.roots;
        for &step in &path {
            let Some(parent) = siblings.get_mut(step) else {
                return Some((section, rest));
            };
            siblings = &mut parent.children;
        }
        siblings.push(rule);
        path.push(siblings.len() - 1);
    }
    Some((section, rest))
}

fn read_rule(bytes: &[u8]) -> Option<(usize, Rule, &[u8])> {
    let (indent, rest) = read_number(bytes);
    let rest = rest.strip_prefix(b">")?;
    let (offset, rest) = read_number(rest);
    let rest = rest.strip_prefix(b"=")?;

    let length = usize::from(u16::from_be_bytes([*rest.first()?, *rest.get(1)?]));
    let rest = rest.get(2..)?;
    let value = rest.get(..length)?.to_vec();
    let mut rest = rest.get(length..)?;

    let mut mask = None;
    if let Some(after) = rest.strip_prefix(b"&") {
        mask = Some(after.get(..length)?.to_vec());
        rest = after.get(length..)?;
    }

    let mut word = 1usize;
    if let Some(after) = rest.strip_prefix(b"~") {
        let (size, after) = read_number(after);
        word = size.max(1);
        rest = after;
    }

    let mut range = 1usize;
    if let Some(after) = rest.strip_prefix(b"+") {
        let (length, after) = read_number(after);
        range = length.max(1);
        rest = after;
    }
    let rest = rest.strip_prefix(b"\n").unwrap_or(rest);

    let mut rule = Rule {
        offset,
        range,
        value,
        mask,
        children: Vec::new(),
    };
    if word > 1 && cfg!(target_endian = "little") {
        swap_words(&mut rule.value, word);
        if let Some(mask) = rule.mask.as_mut() {
            swap_words(mask, word);
        }
    }
    Some((indent, rule, rest))
}

fn swap_words(bytes: &mut [u8], word: usize) {
    for chunk in bytes.chunks_mut(word) {
        chunk.reverse();
    }
}

fn read_number(bytes: &[u8]) -> (usize, &[u8]) {
    let end = bytes
        .iter()
        .position(|byte| !byte.is_ascii_digit())
        .unwrap_or(bytes.len());
    let digits = std::str::from_utf8(&bytes[..end]).unwrap_or_default();
    (digits.parse().unwrap_or(0), &bytes[end..])
}

use std::fmt;
use std::str::FromStr;

const ALPHABET: &[u8; 26] = b"abcdefghijklmnopqrstuvwxyz";
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
const SEPARATOR: char = '#';

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Tag([u8; 2]);

impl Tag {
    pub(crate) fn of(line: usize, content: &str) -> Tag {
        let seeded = (line as u64).to_le_bytes().iter().fold(FNV_OFFSET, fnv);
        let hash = content.as_bytes().iter().fold(seeded, fnv);
        let mixed = hash ^ (hash >> 32);
        let base = ALPHABET.len() as u64;
        Tag([
            ALPHABET[(mixed % base) as usize],
            ALPHABET[(mixed / base % base) as usize],
        ])
    }

    fn parse(text: &str) -> Option<Tag> {
        match text.as_bytes() {
            &[first, second] if first.is_ascii_lowercase() && second.is_ascii_lowercase() => {
                Some(Tag([first, second]))
            }
            _ => None,
        }
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}{}",
            char::from(self.0[0]),
            char::from(self.0[1])
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Anchor {
    line: usize,
    tag: Option<Tag>,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("invalid anchor `{0}`: expected a line number and its tag, like `12#ab`")]
pub struct InvalidAnchor(String);

impl Anchor {
    pub(crate) fn of(line: usize, content: &str) -> Anchor {
        Anchor {
            line,
            tag: Some(Tag::of(line, content)),
        }
    }

    #[must_use]
    pub fn line(&self) -> usize {
        self.line
    }

    pub(crate) fn is_tagged(&self) -> bool {
        self.tag.is_some()
    }

    pub(crate) fn matches(&self, lines: &[&str]) -> bool {
        let content = lines.get(self.line - 1);
        self.tag
            .zip(content)
            .is_some_and(|(tag, content)| tag == Tag::of(self.line, content))
    }
}

impl FromStr for Anchor {
    type Err = InvalidAnchor;

    fn from_str(text: &str) -> Result<Anchor, InvalidAnchor> {
        let invalid = || InvalidAnchor(text.to_owned());
        let (number, tag) = match text.trim().split_once(SEPARATOR) {
            Some((number, tag)) => (number, Some(Tag::parse(tag).ok_or_else(invalid)?)),
            None => (text.trim(), None),
        };
        let line = number
            .parse::<usize>()
            .ok()
            .filter(|&line| line > 0 && number.bytes().all(|byte| byte.is_ascii_digit()))
            .ok_or_else(invalid)?;
        Ok(Anchor { line, tag })
    }
}

impl fmt::Display for Anchor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.tag {
            Some(tag) => write!(formatter, "{}{SEPARATOR}{tag}", self.line),
            None => write!(formatter, "{}", self.line),
        }
    }
}

pub(crate) fn starts_with_anchor(line: &str) -> bool {
    let rest = line.trim_start_matches(' ');
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    let Some(rest) = rest[digits..]
        .strip_prefix(SEPARATOR)
        .filter(|_| digits > 0)
    else {
        return false;
    };
    matches!(
        rest.as_bytes(),
        [first, second, b'\t' | b':' | b'-', ..]
            if first.is_ascii_lowercase() && second.is_ascii_lowercase()
    )
}

fn fnv(hash: u64, byte: &u8) -> u64 {
    (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
}

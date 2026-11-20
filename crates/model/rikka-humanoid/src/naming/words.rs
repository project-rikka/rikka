#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Side {
    Left,
    Right,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Words {
    pub(crate) side: Option<Side>,
    pub(crate) core: Vec<String>,
}

const NOISE: &[&str] = &[
    "mixamorig",
    "bip",
    "biped",
    "valvebiped",
    "cc",
    "base",
    "j",
    "adj",
    "c",
    "def",
    "deform",
    "jnt",
    "joint",
    "bone",
    "bn",
    "sk",
    "armature",
    "rig",
];

const DISQUALIFYING: &[&str] = &[
    "org",
    "mch",
    "ctrl",
    "ctl",
    "control",
    "ik",
    "fk",
    "pole",
    "target",
    "tgt",
    "twist",
    "roll",
    "helper",
    "end",
    "nub",
    "tip",
    "top",
    "dummy",
    "socket",
    "null",
    "attach",
    "offset",
    "corrective",
    "d",
    "sec",
    "share",
    "parent",
    "ref",
];

const LEFT: &[&str] = &["l", "left", "lft"];
const RIGHT: &[&str] = &["r", "right", "rgt"];
const LEFT_MARK: char = '左';
const RIGHT_MARK: char = '右';

pub(crate) fn words(name: &str) -> Option<Words> {
    let last = name.rsplit([':', '|']).next().unwrap_or(name);
    let widened = narrow(last);
    let mut tokens = split(&widened);
    let mut side = None;
    let mut sides = 0;
    let mut marked = |side_now: Side, side: &mut Option<Side>| {
        if *side != Some(side_now) {
            sides += 1;
        }
        *side = Some(side_now);
    };
    for token in &mut tokens {
        if let Some(rest) = token.strip_prefix(LEFT_MARK) {
            marked(Side::Left, &mut side);
            *token = rest.to_owned();
        } else if let Some(rest) = token.strip_prefix(RIGHT_MARK) {
            marked(Side::Right, &mut side);
            *token = rest.to_owned();
        }
        if let Some(rest) = token.strip_suffix(LEFT_MARK) {
            marked(Side::Left, &mut side);
            *token = rest.to_owned();
        } else if let Some(rest) = token.strip_suffix(RIGHT_MARK) {
            marked(Side::Right, &mut side);
            *token = rest.to_owned();
        }
    }
    let mut core = Vec::with_capacity(tokens.len());
    for token in tokens.into_iter().filter(|token| !token.is_empty()) {
        if LEFT.contains(&token.as_str()) {
            marked(Side::Left, &mut side);
        } else if RIGHT.contains(&token.as_str()) {
            marked(Side::Right, &mut side);
        } else if DISQUALIFYING.contains(&token.as_str()) {
            return None;
        } else if NOISE.contains(&token.as_str())
            || (is_number(&token) && !core.iter().any(|word: &String| !is_number(word)))
        {
            continue;
        } else {
            core.push(token);
        }
    }
    if sides > 1 || core.is_empty() {
        return None;
    }
    Some(Words { side, core })
}

pub(crate) fn is_number(token: &str) -> bool {
    !token.is_empty() && token.chars().all(|letter| letter.is_ascii_digit())
}

fn narrow(name: &str) -> String {
    name.trim()
        .chars()
        .map(|letter| match u32::from(letter) {
            code @ 0xFF01..=0xFF5E => char::from_u32(code - 0xFEE0).unwrap_or(letter),
            0x3000 => ' ',
            _ => letter,
        })
        .collect()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Lower,
    Upper,
    Digit,
    Other,
}

fn class(letter: char) -> Option<Class> {
    if letter.is_ascii_lowercase() {
        Some(Class::Lower)
    } else if letter.is_ascii_uppercase() {
        Some(Class::Upper)
    } else if letter.is_ascii_digit() {
        Some(Class::Digit)
    } else if letter.is_alphanumeric() {
        Some(Class::Other)
    } else {
        None
    }
}

fn split(name: &str) -> Vec<String> {
    let letters: Vec<char> = name.chars().collect();
    let mut tokens = Vec::new();
    let mut current = String::new();
    for (at, letter) in letters.iter().copied().enumerate() {
        let Some(now) = class(letter) else {
            flush(&mut current, &mut tokens);
            continue;
        };
        let before = at
            .checked_sub(1)
            .and_then(|previous| class(letters[previous]));
        let after = letters.get(at + 1).copied().and_then(class);
        let breaks = match (before, now) {
            (None, _) => false,
            (Some(Class::Lower), Class::Upper) => true,
            (Some(Class::Upper), Class::Upper) => after == Some(Class::Lower),
            (Some(Class::Digit), Class::Digit) => false,
            (Some(Class::Digit), _) | (Some(_), Class::Digit) => true,
            (Some(Class::Other), Class::Lower | Class::Upper)
            | (Some(Class::Lower | Class::Upper), Class::Other) => true,
            _ => false,
        };
        if breaks {
            flush(&mut current, &mut tokens);
        }
        current.extend(letter.to_lowercase());
    }
    flush(&mut current, &mut tokens);
    tokens
}

fn flush(current: &mut String, tokens: &mut Vec<String>) {
    if !current.is_empty() {
        tokens.push(std::mem::take(current));
    }
}

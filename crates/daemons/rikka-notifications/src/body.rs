use std::ops::Range;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Body {
    pub text: String,
    pub emphasis: Vec<Emphasis>,
    pub links: Vec<Link>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Emphasis {
    pub range: Range<usize>,
    pub style: Style,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Style {
    Bold,
    Italic,
    Underline,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Link {
    pub range: Range<usize>,
    pub href: String,
}

impl Body {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }
}

#[must_use]
pub(crate) fn drawable(character: char) -> bool {
    match character {
        '\n' => true,
        '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' => false,
        other => !other.is_control(),
    }
}

const MAX_CHARS: usize = 320;
const MAX_LINES: usize = 6;

const ELLIPSIS: char = '…';

#[must_use]
pub(crate) fn capped(mut body: Body) -> Body {
    let Some(cut) = cut_at(&body.text) else {
        return body;
    };
    body.text.truncate(cut);
    body.emphasis
        .retain_mut(|emphasis| shorten(&mut emphasis.range, cut));
    body.links.retain_mut(|link| shorten(&mut link.range, cut));
    body.text.push(ELLIPSIS);
    body
}

fn cut_at(text: &str) -> Option<usize> {
    let mut lines = 1;
    for (count, (at, character)) in text.char_indices().enumerate() {
        if count == MAX_CHARS {
            return Some(at);
        }
        if character == '\n' {
            lines += 1;
            if lines > MAX_LINES {
                return Some(at);
            }
        }
    }
    None
}

fn shorten(range: &mut Range<usize>, cut: usize) -> bool {
    range.end = range.end.min(cut);
    range.start < range.end
}

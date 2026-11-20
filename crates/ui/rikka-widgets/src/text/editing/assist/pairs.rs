use rikka_ui::text::Content;

use crate::text::editing::assist::indent::unit_of;

const PAIRS: [(char, char); 3] = [('(', ')'), ('[', ']'), ('{', '}')];

const CLOSES_BEFORE: &str = ";:.,=}])>";

pub(crate) fn closing(open: char) -> Option<char> {
    PAIRS
        .iter()
        .find(|(opens, _)| *opens == open)
        .map(|(_, closes)| *closes)
}

pub(crate) fn is_closer(letter: char) -> bool {
    PAIRS.iter().any(|(_, closes)| *closes == letter)
}

pub(crate) fn after(content: &Content, at: usize) -> Option<char> {
    content.rope().char_at(at)
}

pub(crate) fn before(content: &Content, at: usize) -> Option<char> {
    content.rope().char_before(at)
}

fn closes_well(next: Option<char>) -> bool {
    next.is_none_or(|next| next.is_whitespace() || CLOSES_BEFORE.contains(next))
}

pub(crate) struct Typing<'a> {
    pub(crate) quotes: &'a str,
    pub(crate) prose: bool,
    pub(crate) fallback: usize,
}

impl Typing<'_> {
    pub(crate) fn typed(&self, content: &mut Content, letter: char) -> Option<bool> {
        let quote = self.quotes.contains(letter);
        let selection = content.selection();
        if !selection.is_empty() {
            let close = closing(letter).or(quote.then_some(letter))?;
            return Some(surround(content, letter, close));
        }
        let caret = content.caret();
        let next = after(content, caret);
        if (is_closer(letter) || quote) && next == Some(letter) {
            content.interrupt();
            content.place_caret(caret + letter.len_utf8(), false);
            return Some(false);
        }
        if let Some(close) = closing(letter)
            && closes_well(next)
        {
            return Some(pair(content, letter, close));
        }
        if quote && !self.prose && closes_well(next) && self.opens_quote(content, letter) {
            return Some(pair(content, letter, letter));
        }
        if is_closer(letter) {
            return self.outdented(content, letter);
        }
        None
    }

    fn opens_quote(&self, content: &Content, letter: char) -> bool {
        before(content, content.caret()).is_none_or(|previous| {
            !previous.is_alphanumeric() && previous != '_' && previous != letter
        })
    }

    fn outdented(&self, content: &mut Content, letter: char) -> Option<bool> {
        let caret = content.caret();
        let start = content.line_start(content.line_of(caret));
        let leading = content.slice(start..caret);
        if leading.is_empty() || !leading.chars().all(|ch| ch == ' ' || ch == '\t') {
            return None;
        }
        let unit = unit_of(content, self.fallback);
        let stripped = match leading.strip_suffix(unit.as_str()) {
            Some(stripped) => stripped.to_owned(),
            None => leading
                .trim_end_matches(' ')
                .trim_end_matches('\t')
                .to_owned(),
        };
        let mut inserted = stripped;
        inserted.push(letter);
        Some(content.replace_each(&[(start..caret, inserted)]))
    }
}

pub(crate) fn backspace(content: &mut Content, quotes: &str) -> Option<bool> {
    if content.has_selection() {
        return None;
    }
    let caret = content.caret();
    let previous = before(content, caret)?;
    let next = after(content, caret)?;
    let paired = closing(previous) == Some(next) || (quotes.contains(previous) && previous == next);
    if !paired {
        return None;
    }
    let start = caret - previous.len_utf8();
    let end = caret + next.len_utf8();
    Some(content.replace_each(&[(start..end, String::new())]))
}

fn pair(content: &mut Content, open: char, close: char) -> bool {
    let caret = content.caret();
    let inserted = format!("{open}{close}");
    let changed = content.replace_each(&[(caret..caret, inserted)]);
    content.place_caret(caret + open.len_utf8(), false);
    changed
}

fn surround(content: &mut Content, open: char, close: char) -> bool {
    let selection = content.selection();
    let forward = content.caret() >= content.anchor();
    let changed = content.replace_each(&[
        (selection.start..selection.start, open.to_string()),
        (selection.end..selection.end, close.to_string()),
    ]);
    let start = selection.start + open.len_utf8();
    let end = selection.end + open.len_utf8();
    match forward {
        true => content.select(start, end),
        false => content.select(end, start),
    };
    changed
}

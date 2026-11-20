use std::rc::Rc;

const INVISIBLE: [char; 6] = [
    '\u{200b}', '\u{200c}', '\u{034f}', '\u{feff}', '\u{00ad}', '\u{2060}',
];
const MOST_BREAKS: usize = 2;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Style {
    pub(super) strong: bool,
    pub(super) emphasis: bool,
    pub(super) strike: bool,
    pub(super) code: bool,
    pub(super) link: Option<Rc<str>>,
}

#[derive(Debug)]
pub(super) enum Piece {
    Text { text: String, style: Style },
    Break,
}

#[derive(Debug, Default)]
pub(super) struct Phrasing {
    pub(super) pieces: Vec<Piece>,
}

impl Phrasing {
    pub(super) fn has_breaks(&self) -> bool {
        self.pieces
            .iter()
            .any(|piece| matches!(piece, Piece::Break))
    }

    pub(super) fn joined(parts: impl IntoIterator<Item = Phrasing>, separator: &str) -> Phrasing {
        let mut pieces = Vec::new();
        for part in parts {
            if !pieces.is_empty() {
                pieces.push(Piece::Text {
                    text: separator.to_owned(),
                    style: Style::default(),
                });
            }
            pieces.extend(part.pieces);
        }
        Phrasing { pieces }
    }
}

#[derive(Debug, Default)]
pub(super) struct Gathering {
    pieces: Vec<Piece>,
    space: bool,
    invisible: String,
    breaks: usize,
}

impl Gathering {
    pub(super) fn text(&mut self, text: &str, style: &Style) {
        for character in text.chars() {
            if is_space(character) {
                self.space = true;
                self.invisible.clear();
            } else if INVISIBLE.contains(&character) {
                if !self.space {
                    self.invisible.push(character);
                }
            } else {
                self.visible(character, style);
            }
        }
    }

    pub(super) fn line_break(&mut self) {
        if !self.pieces.is_empty() {
            self.breaks += 1;
        }
        self.space = false;
        self.invisible.clear();
    }

    pub(super) fn finish(&mut self) -> Option<Phrasing> {
        let pieces = std::mem::take(&mut self.pieces);
        *self = Gathering::default();
        (!pieces.is_empty()).then_some(Phrasing { pieces })
    }

    fn visible(&mut self, character: char, style: &Style) {
        if self.breaks > 0 {
            for _ in 0..self.breaks.min(MOST_BREAKS) {
                self.pieces.push(Piece::Break);
            }
            self.breaks = 0;
        } else if self.space && !self.pieces.is_empty() {
            self.push(' ', style);
        }
        self.space = false;
        for invisible in std::mem::take(&mut self.invisible).chars() {
            self.push(invisible, style);
        }
        self.push(character, style);
    }

    fn push(&mut self, character: char, style: &Style) {
        if let Some(Piece::Text { text, style: last }) = self.pieces.last_mut()
            && last == style
        {
            text.push(character);
            return;
        }
        self.pieces.push(Piece::Text {
            text: character.to_string(),
            style: style.clone(),
        });
    }
}

fn is_space(character: char) -> bool {
    matches!(character, ' ' | '\t' | '\n' | '\r' | '\u{c}' | '\u{a0}')
}

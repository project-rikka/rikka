use rikka_syntax::{Language, Lexicon, Suggestion, is_word};
use rikka_types::Rect;
use rikka_ui::text::Content;

pub(crate) const MOST: usize = 64;
pub(crate) const SHOWN: usize = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Cause {
    Typed(char),
    Deleted,
    Asked,
}

#[derive(Debug)]
pub(crate) struct Completing {
    pub(crate) start: usize,
    lexicon: Lexicon,
    pub(crate) offered: Vec<Suggestion>,
    pub(crate) chosen: usize,
    pub(crate) top: usize,
    pub(crate) lingers: bool,
    pub(crate) trigger: (u64, usize),
    pub(crate) width: Option<f32>,
    pub(crate) panel: Option<Rect>,
    pub(crate) unseen: bool,
}

impl Completing {
    pub(crate) fn new(start: usize, lexicon: Lexicon, lingers: bool) -> Completing {
        Completing {
            start,
            lexicon,
            offered: Vec::new(),
            chosen: 0,
            top: 0,
            lingers,
            trigger: (0, 0),
            width: None,
            panel: None,
            unseen: true,
        }
    }

    pub(crate) fn narrow(&mut self, typed: &str, trigger: (u64, usize)) -> bool {
        let previous_choice = self
            .offered
            .get(self.chosen)
            .map(|chosen| chosen.word().to_owned());
        self.offered = self.lexicon.suggest(typed, MOST);
        self.trigger = trigger;
        self.width = None;
        self.chosen = previous_choice
            .and_then(|previous_choice| {
                self.offered
                    .iter()
                    .position(|suggestion| suggestion.word() == previous_choice)
            })
            .filter(|_| !typed.is_empty())
            .unwrap_or(0);
        self.top = 0;
        self.reveal();
        !self.offered.is_empty()
    }

    pub(crate) fn chosen(&self) -> Option<&Suggestion> {
        self.offered.get(self.chosen)
    }

    pub(crate) fn step(&mut self, by: isize) {
        let count = self.offered.len();
        if count == 0 {
            return;
        }
        let at = self.chosen as isize + by;
        self.chosen = match (by.unsigned_abs() > 1, at) {
            (true, at) => at.clamp(0, count as isize - 1) as usize,
            (false, at) if at < 0 => count - 1,
            (false, at) if at as usize >= count => 0,
            (false, at) => at as usize,
        };
        self.reveal();
    }

    pub(crate) fn scroll(&mut self, rows: isize) {
        let furthest = self.offered.len().saturating_sub(SHOWN);
        self.top = (self.top as isize + rows).clamp(0, furthest as isize) as usize;
    }

    fn reveal(&mut self) {
        if self.chosen < self.top {
            self.top = self.chosen;
        }
        if self.chosen >= self.top + SHOWN {
            self.top = self.chosen + 1 - SHOWN;
        }
    }

    pub(crate) fn ghost(&self, typed: &str) -> String {
        self.chosen()
            .and_then(|chosen| chosen.word().strip_prefix(typed))
            .unwrap_or_default()
            .to_owned()
    }
}

pub(crate) fn word_start(content: &Content, caret: usize, language: Option<Language>) -> usize {
    let rope = content.rope();
    let caret = caret.min(rope.len());
    rope.chars_before(caret)
        .take_while(|letter| is_word(language, *letter))
        .fold(caret, |start, letter| start - letter.len_utf8())
}

pub(crate) fn reaches_into(content: &Content, caret: usize, letter: char) -> bool {
    let previous = content.rope().chars_before(caret).nth(1);
    match letter {
        '.' => true,
        ':' => previous == Some(':'),
        '>' => previous == Some('-'),
        _ => false,
    }
}

use std::time::{Duration, Instant};

use crate::navigation::menu::MenuItem;

const PAUSE: Duration = Duration::from_secs(1);

#[derive(Debug, Default)]
pub(crate) struct Typeahead {
    typed: String,
    last: Option<Instant>,
}

impl Typeahead {
    pub(crate) fn is_typing(&self, now: Instant) -> bool {
        !self.typed.is_empty()
            && self
                .last
                .is_some_and(|last| now.saturating_duration_since(last) < PAUSE)
    }

    pub(crate) fn push(&mut self, letter: char, now: Instant) {
        if !self.is_typing(now) {
            self.typed.clear();
        }
        self.typed.extend(letter.to_lowercase());
        self.last = Some(now);
    }

    pub(crate) fn find(&self, items: &[MenuItem], from: Option<usize>) -> Option<usize> {
        let mut letters = self.typed.chars();
        let first = letters.next()?;
        let repeated = letters.all(|letter| letter == first);
        let wanted = match repeated {
            true => &self.typed[..first.len_utf8()],
            false => self.typed.as_str(),
        };
        let count = items.len();
        let start = match (from, repeated) {
            (Some(row), true) => row + 1,
            (Some(row), false) => row,
            (None, _) => 0,
        };
        (0..count).map(|step| (start + step) % count).find(|row| {
            let item = &items[*row];
            item.is_command()
                && item
                    .label
                    .as_str()
                    .trim_start()
                    .to_lowercase()
                    .starts_with(wanted)
        })
    }
}

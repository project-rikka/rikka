use std::cmp::Ordering;

#[must_use]
pub fn natural_cmp(left: &str, right: &str) -> Ordering {
    let mut tiebreak = Ordering::Equal;
    let mut left = Runs::new(left);
    let mut right = Runs::new(right);

    loop {
        match (left.next(), right.next()) {
            (None, None) => return tiebreak,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(one), Some(other)) => match one.cmp_with(&other) {
                (Ordering::Equal, weak) => tiebreak = tiebreak.then(weak),
                (decided, _) => return decided,
            },
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Run<'a> {
    Number { digits: &'a str, zeros: usize },
    Text(&'a str),
}

impl Run<'_> {
    fn cmp_with(&self, other: &Self) -> (Ordering, Ordering) {
        match (self, other) {
            (
                Self::Number { digits, zeros },
                Self::Number {
                    digits: theirs,
                    zeros: their_zeros,
                },
            ) => {
                let decided = digits
                    .len()
                    .cmp(&theirs.len())
                    .then_with(|| digits.cmp(theirs));
                (decided, zeros.cmp(their_zeros))
            }
            (Self::Text(text), Self::Text(theirs)) => {
                let folded = fold(text).cmp(fold(theirs));
                (folded, text.cmp(theirs))
            }
            (Self::Number { .. }, Self::Text(_)) => (Ordering::Less, Ordering::Equal),
            (Self::Text(_), Self::Number { .. }) => (Ordering::Greater, Ordering::Equal),
        }
    }
}

fn fold(text: &str) -> impl Iterator<Item = char> + '_ {
    text.chars().flat_map(char::to_lowercase)
}

struct Runs<'a> {
    rest: &'a str,
}

impl<'a> Runs<'a> {
    fn new(name: &'a str) -> Self {
        Self { rest: name }
    }
}

impl<'a> Iterator for Runs<'a> {
    type Item = Run<'a>;

    fn next(&mut self) -> Option<Run<'a>> {
        let first = self.rest.chars().next()?;
        if first.is_ascii_digit() {
            let end = self
                .rest
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(self.rest.len());
            let (digits, rest) = self.rest.split_at(end);
            self.rest = rest;
            let significant = digits.trim_start_matches('0');
            Some(Run::Number {
                zeros: digits.len() - significant.len(),
                digits: significant,
            })
        } else {
            let end = self
                .rest
                .find(|c: char| c.is_ascii_digit())
                .unwrap_or(self.rest.len());
            let (text, rest) = self.rest.split_at(end);
            self.rest = rest;
            Some(Run::Text(text))
        }
    }
}

#[must_use]
pub fn matches_glob(pattern: &str, subject: &str) -> bool {
    walk(pattern, subject, Case::Exact)
}

#[must_use]
pub fn matches_glob_folding_case(pattern: &str, subject: &str) -> bool {
    walk(pattern, subject, Case::Fold)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Case {
    Exact,
    Fold,
}

impl Case {
    fn fold(self, c: char) -> char {
        match self {
            Self::Exact => c,
            Self::Fold => c.to_ascii_lowercase(),
        }
    }

    fn same(self, one: char, other: char) -> bool {
        self.fold(one) == self.fold(other)
    }
}

fn walk(pattern: &str, subject: &str, case: Case) -> bool {
    let mut at = 0;
    let mut index = 0;
    let mut star: Option<(usize, usize)> = None;

    while let Some(c) = subject[index..].chars().next() {
        let step = match pattern[at..].chars().next() {
            Some('*') => {
                star = Some((at, index));
                at += 1;
                continue;
            }
            Some('?') => Some(at + 1),
            Some('[') => match bracket(pattern, at, c, case) {
                Some((after, true)) => Some(after),
                Some((_, false)) => None,
                None => case.same('[', c).then_some(at + 1),
            },
            Some(literal) => case.same(literal, c).then_some(at + literal.len_utf8()),
            None => None,
        };
        match step {
            Some(next) => {
                at = next;
                index += c.len_utf8();
            }
            None => match star {
                Some((mark, from)) => {
                    at = mark + 1;
                    index = from + subject[from..].chars().next().map_or(0, char::len_utf8);
                    star = Some((mark, index));
                }
                None => return false,
            },
        }
    }
    pattern[at..].chars().all(|c| c == '*')
}

fn bracket(pattern: &str, open: usize, c: char, case: Case) -> Option<(usize, bool)> {
    let mut at = open + 1;
    let negated = matches!(pattern[at..].chars().next(), Some('!' | '^'));
    if negated {
        at += 1;
    }

    let mut matched = false;
    let mut first = true;
    loop {
        let item = pattern[at..].chars().next()?;
        if item == ']' && !first {
            break;
        }
        first = false;

        let after = at + item.len_utf8();
        let range = pattern[after..]
            .starts_with('-')
            .then(|| pattern[after + 1..].chars().next())
            .flatten()
            .filter(|&end| end != ']');
        match range {
            Some(end) => {
                if (case.fold(item)..=case.fold(end)).contains(&case.fold(c)) {
                    matched = true;
                }
                at = after + 1 + end.len_utf8();
            }
            None => {
                if case.same(item, c) {
                    matched = true;
                }
                at = after;
            }
        }
    }
    Some((at + 1, matched != negated))
}

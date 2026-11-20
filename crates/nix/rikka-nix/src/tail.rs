#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tail {
    next: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("transcript skips from byte {expected} to byte {found}")]
pub struct Gap {
    pub expected: u64,
    pub found: u64,
}

impl Tail {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn next(self) -> u64 {
        self.next
    }

    pub fn accept<'a>(&mut self, from: u64, lines: &'a [String]) -> Result<&'a [String], Gap> {
        if from > self.next {
            return Err(Gap {
                expected: self.next,
                found: from,
            });
        }
        let mut at = from;
        let seen = lines
            .iter()
            .take_while(|line| {
                let old = at < self.next;
                if old {
                    at += span(line);
                }
                old
            })
            .count();
        let fresh = &lines[seen..];
        self.next = self.next.max(at) + fresh.iter().map(|line| span(line)).sum::<u64>();
        Ok(fresh)
    }
}

fn span(line: &str) -> u64 {
    line.len() as u64 + 1
}

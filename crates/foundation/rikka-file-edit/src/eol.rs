use std::ops::Range;

use crate::placement::Site;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Eol {
    #[default]
    Lf,
    Crlf,
}

impl Eol {
    pub(crate) fn detect(text: &str) -> Eol {
        match text
            .match_indices('\n')
            .all(|(at, _)| at > 0 && text.as_bytes()[at - 1] == b'\r')
        {
            true if text.contains('\n') => Eol::Crlf,
            _ => Eol::Lf,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Returns {
    returned: Vec<bool>,
}

impl Returns {
    pub(crate) fn strip<'a>(lines: impl Iterator<Item = &'a str>) -> (Vec<&'a str>, Returns) {
        let mut returns = Returns::default();
        let stripped = lines
            .map(|line| {
                let stripped = line.strip_suffix('\r');
                returns.returned.push(stripped.is_some());
                stripped.unwrap_or(line)
            })
            .collect();
        (stripped, returns)
    }

    pub(crate) fn extend_from(&mut self, source: &Returns, lines: Range<usize>) {
        self.returned.extend_from_slice(&source.returned[lines]);
    }

    pub(crate) fn extend_replacing(
        &mut self,
        source: &Returns,
        old: &Range<usize>,
        site: Site,
        count: usize,
    ) {
        let returned = |line: usize| source.returned.get(line).copied().unwrap_or(false);
        self.returned.extend((0..count).map(|nth| match site {
            Site::Lines => returned(old.start + nth.min(old.len() - 1)),
            Site::After => returned(old.start - 1),
            Site::Before => returned(old.start),
        }));
    }

    pub(crate) fn join(&self, lines: &[&str], newline: bool) -> String {
        let mut joined = String::with_capacity(lines.iter().map(|line| line.len() + 2).sum());
        for (index, line) in lines.iter().enumerate() {
            joined.push_str(line);
            if self.returned[index] {
                joined.push('\r');
            }
            if newline || index + 1 < lines.len() {
                joined.push('\n');
            }
        }
        joined
    }
}

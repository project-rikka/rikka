use crate::cut::Cut;

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

#[derive(Debug)]
pub(crate) struct Returns {
    newlines: Vec<usize>,
}

impl Returns {
    pub(crate) fn strip(content: &str) -> (String, Returns) {
        let mut stripped = String::with_capacity(content.len());
        let mut newlines = Vec::new();
        let mut from = 0;
        for (at, _) in content.match_indices("\r\n") {
            stripped.push_str(&content[from..at]);
            newlines.push(stripped.len());
            stripped.push('\n');
            from = at + 2;
        }
        stripped.push_str(&content[from..]);
        (stripped, Returns { newlines })
    }

    pub(crate) fn replace(&mut self, cut: &Cut, old: &str, new: &str) {
        let returned: Vec<bool> = old
            .match_indices('\n')
            .map(|(at, _)| self.newlines.binary_search(&(cut.at.start + at)).is_ok())
            .collect();
        let written = new
            .match_indices('\n')
            .enumerate()
            .filter(|(index, _)| {
                returned
                    .get(*index)
                    .or(returned.last())
                    .is_some_and(|&returned| returned)
            })
            .map(|(_, (at, _))| cut.at.start + at);
        let (before, after): (Vec<usize>, Vec<usize>) = self
            .newlines
            .iter()
            .filter_map(|&at| cut.moved(at))
            .partition(|&at| at < cut.at.start);
        self.newlines = before.into_iter().chain(written).chain(after).collect();
    }

    pub(crate) fn restore(&self, content: &str) -> String {
        let mut restored = String::with_capacity(content.len() + self.newlines.len());
        let mut from = 0;
        for &at in &self.newlines {
            restored.push_str(&content[from..at]);
            restored.push('\r');
            from = at;
        }
        restored.push_str(&content[from..]);
        restored
    }
}

use serde::Deserialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Step {
    Push,
    Replace,
    Reload,
    Traverse,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Known {
    pub(crate) back: usize,
    pub(crate) forward: usize,
}

#[derive(Debug)]
struct Awaiting {
    url: String,
    entries: Vec<Option<String>>,
    index: usize,
}

#[derive(Debug, Default)]
pub(crate) struct JointHistory {
    entries: Vec<Option<String>>,
    index: usize,
    known: Known,
    awaiting: Option<Awaiting>,
    reported: Option<String>,
}

impl JointHistory {
    pub(crate) fn committed(&mut self, url: &str, step: Step) {
        if self.reported.take().as_deref() == Some(url) {
            return;
        }
        self.awaiting = Some(Awaiting {
            url: url.to_owned(),
            entries: self.entries.clone(),
            index: self.index,
        });
        self.known = Known::default();
        self.apply(url, step);
    }

    pub(crate) fn reported(&mut self, url: &str, step: Option<Step>, length: usize, known: Known) {
        if let Some(step) = step {
            match self.awaiting.take() {
                Some(awaiting) if awaiting.url == url => {
                    self.entries = awaiting.entries;
                    self.index = awaiting.index;
                }
                _ => self.reported = Some(url.to_owned()),
            }
            self.apply(url, step);
        }
        self.known = known;
        self.fit(length.max(1));
    }

    pub(crate) fn can_go_back(&self) -> bool {
        self.index > 0 || self.known.back > 0
    }

    pub(crate) fn can_go_forward(&self) -> bool {
        self.index + 1 < self.entries.len() || self.known.forward > 0
    }

    fn apply(&mut self, url: &str, step: Step) {
        let entry = Some(url.to_owned());
        if self.entries.is_empty() {
            self.entries.push(entry);
            self.index = 0;
            return;
        }
        match step {
            Step::Push => {
                self.entries.truncate(self.index + 1);
                self.entries.push(entry);
                self.index = self.entries.len() - 1;
            }
            Step::Replace | Step::Reload => self.entries[self.index] = entry,
            Step::Traverse => {
                self.index = self.destination(url);
                self.entries[self.index] = entry;
            }
        }
    }

    fn destination(&self, url: &str) -> usize {
        let matching = (1..self.entries.len()).find_map(|distance| {
            let back = self.index.checked_sub(distance);
            let forward = Some(self.index + distance).filter(|at| *at < self.entries.len());
            [back, forward]
                .into_iter()
                .flatten()
                .find(|at| self.entries[*at].as_deref() == Some(url))
        });
        matching.unwrap_or(match self.index {
            0 => (self.index + 1).min(self.entries.len() - 1),
            index => index - 1,
        })
    }

    fn fit(&mut self, length: usize) {
        let count = self.entries.len();
        if count == 0 {
            self.entries = vec![None; length];
            self.index = length - 1;
        } else if length > count {
            let missing = length - count;
            self.entries
                .splice(0..0, std::iter::repeat_n(None, missing));
            self.index += missing;
        } else if length < count {
            let excess = count - length;
            let front = excess.min(self.index);
            self.entries.drain(..front);
            self.index -= front;
            self.entries.truncate(length);
            self.index = self.index.min(length - 1);
        }
        let last = self.entries.len() - 1;
        if self.index + self.known.forward > last {
            self.index = last.saturating_sub(self.known.forward);
        }
        if self.index < self.known.back {
            self.index = self.known.back.min(last);
        }
    }
}

const OPEN: &str = "<think>";
const CLOSE: &str = "</think>";

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Split {
    pub(super) reasoning: String,
    pub(super) text: String,
}

#[derive(Debug, Default)]
pub(super) struct Thinking {
    state: State,
    held: String,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum State {
    #[default]
    Opening,
    Inside,
    Outside,
}

impl Thinking {
    pub(super) fn split(&mut self, piece: &str) -> Split {
        let mut split = Split::default();
        if self.state == State::Outside {
            split.text.push_str(piece);
            return split;
        }
        self.held.push_str(piece);
        if self.state == State::Opening {
            let started = self.held.trim_start();
            if let Some(rest) = started.strip_prefix(OPEN) {
                self.held = rest.to_owned();
                self.state = State::Inside;
            } else if OPEN.starts_with(started) {
                return split;
            } else {
                self.state = State::Outside;
                split.text = std::mem::take(&mut self.held);
                return split;
            }
        }
        match self.held.find(CLOSE) {
            Some(at) => {
                split.reasoning = self.held[..at].to_owned();
                split.text = self.held[at + CLOSE.len()..].trim_start().to_owned();
                self.held.clear();
                self.state = State::Outside;
            }
            None => {
                let kept = partial_close(&self.held);
                let cut = self.held.len() - kept;
                split.reasoning = self.held[..cut].to_owned();
                self.held.drain(..cut);
            }
        }
        split
    }

    pub(super) fn finish(&mut self) -> Split {
        let held = std::mem::take(&mut self.held);
        match std::mem::replace(&mut self.state, State::Outside) {
            State::Inside => Split {
                reasoning: held,
                text: String::new(),
            },
            State::Opening | State::Outside => Split {
                reasoning: String::new(),
                text: held,
            },
        }
    }
}

fn partial_close(held: &str) -> usize {
    (1..CLOSE.len())
        .rev()
        .find(|length| held.ends_with(&CLOSE[..*length]))
        .unwrap_or(0)
}

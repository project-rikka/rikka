use animate_cast::Subject;

use super::{Animate, Batch};
use crate::arrival::{Choice, Settled, Usage};

#[derive(Debug)]
pub(crate) struct Choosing {
    choice: Choice,
    usage: Usage,
}

impl Choosing {
    pub(crate) fn choice(&self) -> &Choice {
        &self.choice
    }

    pub(crate) fn usage(&self) -> Usage {
        self.usage
    }
}

impl Animate {
    pub(super) fn choose(&mut self, choice: Choice) {
        let usage = match choice.figure && self.subject_figure().is_none() {
            true => Usage::Figure { clip: None },
            false => Usage::Motion { clip: 0 },
        };
        self.loading.choices.push_back(Choosing { choice, usage });
    }

    pub(crate) fn choosing(&self) -> Option<&Choosing> {
        self.loading.choices.front()
    }

    pub(crate) fn subject_figure(&self) -> Option<&str> {
        let Some(Subject::Figure(id)) = self.subject else {
            return None;
        };
        self.cast.figure(id).map(|figure| figure.name.as_str())
    }

    pub(crate) fn choose_usage(&mut self, as_figure: bool) {
        if let Some(choosing) = self.loading.choices.front_mut() {
            choosing.usage = match (as_figure, choosing.usage) {
                (true, Usage::Motion { clip }) => Usage::Figure { clip: Some(clip) },
                (false, Usage::Figure { clip }) => Usage::Motion {
                    clip: clip.unwrap_or(0),
                },
                (_, usage) => usage,
            };
        }
    }

    pub(crate) fn choose_clip(&mut self, clip: Option<usize>) {
        if let Some(choosing) = self.loading.choices.front_mut() {
            choosing.usage = match (choosing.usage, clip) {
                (Usage::Figure { .. }, clip) => Usage::Figure { clip },
                (Usage::Motion { .. }, Some(clip)) => Usage::Motion { clip },
                (usage, None) => usage,
            };
        }
    }

    pub(crate) fn can_settle(&self) -> bool {
        self.choosing()
            .is_some_and(|choosing| match choosing.usage {
                Usage::Figure { .. } => choosing.choice.figure,
                Usage::Motion { .. } => self.subject_figure().is_some(),
            })
    }

    pub(crate) fn settle_choice(&mut self) {
        if !self.can_settle() {
            return;
        }
        let Some(choosing) = self.loading.choices.pop_front() else {
            return;
        };
        self.loading.queue.push_back(Batch {
            paths: Vec::new(),
            settled: Some(Settled {
                path: choosing.choice.path,
                model: choosing.choice.model,
                usage: choosing.usage,
            }),
        });
        self.next_load();
    }

    pub(crate) fn skip_choice(&mut self) {
        self.loading.choices.pop_front();
    }
}

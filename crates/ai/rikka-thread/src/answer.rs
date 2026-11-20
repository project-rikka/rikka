use std::sync::Arc;

use rikka_llm::{HostedOutcome, HostedPart};

use crate::call::{Call, CallResult};
use crate::reply::Reply;
use crate::round::Round;
use crate::turn::TurnText;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Streaming,
    Running,
    Hosted,
}

#[derive(Clone, Debug)]
pub struct Answer {
    rounds: Vec<Arc<Round>>,
    phase: Phase,
    model: String,
    folding: bool,
}

impl Answer {
    #[must_use]
    pub fn new(model: impl Into<String>) -> Answer {
        Answer {
            rounds: vec![Arc::default()],
            phase: Phase::Streaming,
            model: model.into(),
            folding: false,
        }
    }

    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn apply(&mut self, reply: &Reply) {
        self.folding = matches!(reply, Reply::Compacting);
        match reply {
            Reply::Delta(delta) => {
                if matches!(self.phase, Phase::Running | Phase::Hosted) {
                    self.begin();
                }
                self.open().say(delta);
            }

            Reply::Responded(replay) => self.open().replayed(replay),

            Reply::TurnFinished(_) => {
                if self.phase == Phase::Running {
                    self.rounds.push(Arc::default());
                }
                self.phase = Phase::Running;
            }

            Reply::Started { id, tool, input } => self.open().started(id, tool, input),

            Reply::Finished { id, result } => self.open().set_result(id, result.clone()),

            Reply::Hosted(part) => self.hosted(part),

            Reply::Noticed(notice) => self.open().noticed(notice.clone()),

            Reply::Resumed => self.phase = Phase::Streaming,

            Reply::Compacting | Reply::Compacted(_) | Reply::CompactFailed(_) => {}

            Reply::Interjected { .. } | Reply::Done | Reply::Stopped | Reply::Failed(_) => {}
        }
    }

    #[must_use]
    pub fn is_compacting(&self) -> bool {
        self.folding
    }

    #[must_use]
    pub fn rounds(&self) -> &[Arc<Round>] {
        &self.rounds
    }

    #[must_use]
    pub fn open_round(&self) -> (usize, &str) {
        let round = self.rounds.len() - 1;
        (round, self.rounds[round].text())
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rounds.iter().all(|round| round.is_empty())
    }

    #[must_use]
    pub fn finished(&self) -> (TurnText, Vec<Round>) {
        self.clone().finish()
    }

    #[must_use]
    pub fn finish(self) -> (TurnText, Vec<Round>) {
        let mut rounds: Vec<Round> = self.rounds.into_iter().map(Arc::unwrap_or_clone).collect();
        for round in &mut rounds {
            round.settle();
        }
        rounds.retain(|round| !round.is_empty());

        let ended = rounds.last().is_some_and(Round::is_answer);
        let (text, replay) = match ended {
            true => rounds.pop().map(Round::into_spoken).unwrap_or_default(),
            false => (String::new(), None),
        };

        let reply = TurnText {
            text,
            replay,
            model: self.model,
            ..TurnText::default()
        };
        (reply, rounds)
    }

    fn hosted(&mut self, part: &HostedPart) {
        match part {
            HostedPart::Call { id, tool, input } => {
                if self.phase == Phase::Running {
                    self.begin();
                }
                self.open().add(Call::hosted(
                    Arc::from(id.as_str()),
                    Arc::from(tool.name()),
                    Arc::new(input.clone()),
                ));
                self.phase = Phase::Hosted;
            }
            HostedPart::Result { id, outcome, .. } => {
                let asked = self
                    .rounds
                    .iter_mut()
                    .rev()
                    .find(|round| round.calls().iter().any(|call| call.id() == id));
                if let Some(round) = asked {
                    Arc::make_mut(round).set_result(id, hosted_result(outcome));
                }
            }
        }
    }

    fn begin(&mut self) {
        self.rounds.push(Arc::default());
        self.phase = Phase::Streaming;
    }

    fn open(&mut self) -> &mut Round {
        let round = self
            .rounds
            .last_mut()
            .expect("answer should have an open round");
        Arc::make_mut(round)
    }
}

fn hosted_result(outcome: &HostedOutcome) -> CallResult {
    CallResult {
        output: Arc::from(outcome.to_string()),
        failed: outcome.is_failed(),
        ..CallResult::default()
    }
}

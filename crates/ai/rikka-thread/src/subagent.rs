use std::sync::Arc;

use rikka_llm::ModelInfo;

use crate::answer::Answer;
use crate::ask::Ask;
use crate::reply::Reply;
use crate::thread::Thread;
use crate::turn::{Speaker, Turn};

const KEPT: usize = 4 << 20;

#[derive(Clone, Debug)]
pub struct Subagent {
    thread: Arc<Thread>,
    answering: Option<Answer>,
    model: Option<ModelInfo>,
}

impl Subagent {
    #[must_use]
    pub fn thread(&self) -> &Thread {
        &self.thread
    }

    #[must_use]
    pub fn answer(&self) -> Option<&Answer> {
        self.answering.as_ref()
    }

    #[must_use]
    pub fn model(&self) -> Option<&ModelInfo> {
        self.model.as_ref()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Holding {
    Bounded,
    Whole,
}

#[derive(Debug)]
pub(crate) struct SubagentTranscript {
    thread: Arc<Thread>,
    answering: Option<Answer>,
    holding: Holding,
}

impl SubagentTranscript {
    pub(crate) fn new(thread: Thread, holding: Holding) -> SubagentTranscript {
        SubagentTranscript {
            thread: Arc::new(thread),
            answering: None,
            holding,
        }
    }

    pub(crate) fn thread(&self) -> &Arc<Thread> {
        &self.thread
    }

    pub(crate) fn instructions(&self) -> Option<Vec<String>> {
        let (sent, _) = self.thread.instructed()?;
        sent.instructions().map(<[String]>::to_vec)
    }

    pub(crate) fn push_task(&mut self, task: &str, instructions: Option<Vec<String>>) {
        self.settle();
        self.edit()
            .add_request(Speaker::User, Ask::new(task), instructions);
    }

    pub(crate) fn apply(&mut self, reply: &Reply, model: &str) -> bool {
        match reply {
            Reply::Done | Reply::Stopped | Reply::Failed(_) => {
                self.settle();
                true
            }
            Reply::Compacted(lead) => {
                let lead = lead.clone();
                self.settle();
                self.edit().compact(lead);
                true
            }
            reply => {
                let answering = self.answering.get_or_insert_with(|| Answer::new(model));
                answering.apply(reply);
                match reply {
                    Reply::TurnFinished(usage) => {
                        let usage = *usage;
                        self.edit().record_usage(usage);
                        true
                    }
                    Reply::Finished { .. } => self.keep_partial(),
                    _ => false,
                }
            }
        }
    }

    pub(crate) fn snapshot(&self, model: Option<ModelInfo>) -> Subagent {
        Subagent {
            thread: Arc::clone(&self.thread),
            answering: self.answering.clone(),
            model,
        }
    }

    fn keep_partial(&mut self) -> bool {
        let Some((text, rounds)) = self.answering.as_ref().map(Answer::finished) else {
            return false;
        };
        self.edit().answer_partial(text, rounds);
        true
    }

    fn settle(&mut self) {
        let Some(answer) = self.answering.take() else {
            return;
        };
        let (text, rounds) = answer.finish();
        self.edit().answer(text, rounds);
        if self.holding == Holding::Bounded {
            self.evict_oldest();
        }
    }

    fn evict_oldest(&mut self) {
        let turns = self.thread.all_turns();
        let mut bytes = 0;
        let mut eldest = 0;
        for (at, turn) in turns.iter().enumerate().rev() {
            eldest = at;
            bytes += turn.body().bytes();
            if bytes >= KEPT {
                break;
            }
        }
        if eldest == 0 {
            return;
        }

        let recent: Vec<Arc<Turn>> = turns[eldest..].to_vec();
        self.thread = Arc::new(Thread::from_turns(self.thread.summary(), recent));
    }

    fn edit(&mut self) -> &mut Thread {
        Arc::make_mut(&mut self.thread)
    }
}

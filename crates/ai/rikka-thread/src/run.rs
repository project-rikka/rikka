use rikka_agent::{Interjection, Interjections};
use rikka_types::CancelToken;

use crate::answer::Answer;
use crate::ask::Ask;
use crate::conversation::Conversation;
use crate::reply::Reply;
use crate::round::Round;
use crate::thread::Thread;
use crate::turn::{Speaker, TurnText};

#[derive(Debug, Default)]
pub(crate) struct Run {
    phase: Phase,
    waiting: Vec<QueuedAsk>,
    mailbox: Interjections,
    lead: Option<String>,
    next_id: u64,
}

#[derive(Debug, Default)]
enum Phase {
    #[default]
    Idle,
    Answering {
        answer: Answer,
        cancel: CancelToken,
    },
    Stopped,
}

#[derive(Clone, Debug, PartialEq)]
pub struct QueuedAsk {
    id: u64,
    ask: Ask,
}

impl QueuedAsk {
    #[must_use]
    pub fn id(&self) -> u64 {
        self.id
    }

    #[must_use]
    pub fn ask(&self) -> &Ask {
        &self.ask
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunOutcome {
    AnswerUpdated,
    ThreadUpdated,
    Finished,
}

impl Run {
    pub(crate) fn from_conversation(conversation: &Conversation) -> Run {
        Run {
            phase: Phase::Answering {
                answer: Answer::new(conversation.model()),
                cancel: conversation.cancel_token(),
            },
            waiting: Vec::new(),
            mailbox: conversation.interjections().clone(),
            lead: conversation.lead().map(str::to_owned),
            next_id: 0,
        }
    }

    pub(crate) fn answer(&self) -> Option<&Answer> {
        match &self.phase {
            Phase::Answering { answer, .. } => Some(answer),
            Phase::Idle | Phase::Stopped => None,
        }
    }

    pub(crate) fn is_answering(&self) -> bool {
        matches!(self.phase, Phase::Answering { .. })
    }

    pub(crate) fn finished_answer(&self) -> Option<(TurnText, Vec<Round>)> {
        self.answer().map(Answer::finished)
    }

    pub(crate) fn queued(&self) -> &[QueuedAsk] {
        &self.waiting
    }

    pub(crate) fn interject(&mut self, ask: Ask) -> Result<u64, Ask> {
        let Phase::Answering { cancel, .. } = &self.phase else {
            return Err(ask);
        };
        if cancel.is_cancelled() {
            return Err(ask);
        }
        self.next_id += 1;
        let id = self.next_id;
        let content = ask.clone().led_by(self.lead.clone()).content();
        self.mailbox.post(Interjection::new(id, content));
        self.waiting.push(QueuedAsk { id, ask });
        Ok(id)
    }

    pub(crate) fn withdraw(&mut self, id: u64) -> Option<Ask> {
        let at = self.waiting.iter().position(|waiting| waiting.id == id)?;
        if self.is_answering() && !self.mailbox.withdraw(id) {
            return None;
        }
        Some(self.waiting.remove(at).ask)
    }

    pub(crate) fn recall(&mut self) -> Vec<Ask> {
        let ids: Vec<u64> = self.waiting.iter().map(QueuedAsk::id).collect();
        ids.into_iter().filter_map(|id| self.withdraw(id)).collect()
    }

    pub(crate) fn stop(&mut self) {
        match &self.phase {
            Phase::Answering { cancel, .. } => cancel.cancel(),
            Phase::Idle | Phase::Stopped => self.phase = Phase::Stopped,
        }
    }

    pub(crate) fn apply<'a>(
        &mut self,
        reply: &Reply,
        thread: impl FnOnce() -> Option<&'a mut Thread>,
    ) -> RunOutcome {
        let Phase::Answering { answer, cancel } = &mut self.phase else {
            return RunOutcome::AnswerUpdated;
        };
        match reply {
            Reply::TurnFinished(usage) => {
                answer.apply(reply);
                if let Some(thread) = thread() {
                    thread.record_usage(*usage);
                }
                RunOutcome::ThreadUpdated
            }

            Reply::Compacted(lead) => {
                let fresh = Answer::new(answer.model());
                let (text, rounds) = std::mem::replace(answer, fresh).finish();
                if let Some(thread) = thread() {
                    thread.answer(text, rounds);
                    thread.compact(lead.as_str());
                }
                RunOutcome::ThreadUpdated
            }

            Reply::Interjected { id, at } => {
                let Some(index) = self.waiting.iter().position(|waiting| waiting.id == *id) else {
                    return RunOutcome::AnswerUpdated;
                };
                let ask = self.waiting.remove(index).ask.led_by(self.lead.clone());
                let fresh = Answer::new(answer.model());
                let (text, rounds) = std::mem::replace(answer, fresh).finish();
                if let Some(thread) = thread() {
                    thread.answer(text, rounds);
                    thread.asked_at(Speaker::User, ask, None, *at);
                }
                RunOutcome::ThreadUpdated
            }

            Reply::Done | Reply::Stopped | Reply::Failed(_) => {
                let ended = match cancel.is_cancelled() {
                    true => Phase::Stopped,
                    false => Phase::Idle,
                };
                let unheard = self.mailbox.take();
                self.waiting
                    .retain(|waiting| unheard.iter().any(|left| left.id() == waiting.id));
                if let Phase::Answering { answer, .. } = std::mem::replace(&mut self.phase, ended)
                    && let Some(thread) = thread()
                {
                    let (text, rounds) = answer.finish();
                    thread.answer(text, rounds);
                }
                RunOutcome::Finished
            }

            reply => {
                answer.apply(reply);
                RunOutcome::AnswerUpdated
            }
        }
    }
}

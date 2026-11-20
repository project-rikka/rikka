use rikka_sync::{Mutex, MutexGuard};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::mpsc::{self, Sender};

use futures_channel::oneshot;

use crate::ask::{Ask, Notice, Question};
use crate::conversation;
use crate::helper::Stop;
use crate::identity::{self, User};
use crate::reply::Reply;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Outcome {
    Authorized,
    Denied,
    Cancelled,
}

#[derive(Debug)]
pub(crate) struct Opening {
    pub(crate) action: String,
    pub(crate) message: String,
    pub(crate) details: Vec<(String, String)>,
    pub(crate) users: Vec<User>,
    pub(crate) cookie: String,
}

pub(crate) struct Pending {
    state: Mutex<State>,
    report: Arc<dyn Fn(Option<Box<Ask>>) + Send + Sync>,
}

#[derive(Debug, Default)]
struct State {
    queue: VecDeque<Authentication>,
    asks: u64,
    questions: u64,
}

#[derive(Debug)]
struct Authentication {
    ask: Ask,
    cookie: String,
    replies: Option<Sender<Reply>>,
    interrupt: Option<Stop>,
    waiting: Option<oneshot::Sender<Outcome>>,
    cancelled: bool,
}

impl Pending {
    pub(crate) fn new(report: Arc<dyn Fn(Option<Box<Ask>>) + Send + Sync>) -> Pending {
        Pending {
            state: Mutex::new(State::default()),
            report,
        }
    }

    pub(crate) fn open(self: &Arc<Self>, opening: Opening) -> oneshot::Receiver<Outcome> {
        let (waiting, reply) = oneshot::channel();
        {
            let mut state = self.state();
            state.asks += 1;
            let ask = Ask {
                id: state.asks,
                action: opening.action,
                message: opening.message,
                details: opening.details,
                chosen: identity::preferred(&opening.users),
                users: opening.users,
                question: None,
                notice: None,
                failures: 0,
            };
            state.queue.push_back(Authentication {
                ask,
                cookie: opening.cookie,
                replies: None,
                interrupt: None,
                waiting: Some(waiting),
                cancelled: false,
            });
        }
        self.turn();
        reply
    }

    pub(crate) fn answer(self: &Arc<Self>, id: u64, reply: Reply) {
        if matches!(reply, Reply::Dismiss) {
            self.call_off(|found| found.ask.id == id);
            return;
        }
        if let Reply::Choose(index) = reply
            && !self.chose(id, index)
        {
            return;
        }
        let replies = {
            let state = self.state();
            state
                .queue
                .front()
                .filter(|found| found.ask.id == id)
                .and_then(|found| found.replies.clone())
        };
        if let Some(replies) = replies {
            let _ = replies.send(reply);
        }
    }

    pub(crate) fn withdraw(self: &Arc<Self>, cookie: &str) {
        self.call_off(|found| found.cookie == cookie);
    }

    pub(crate) fn close(self: &Arc<Self>) {
        self.call_off(|_| true);
    }

    pub(crate) fn cancelled(&self, id: u64) -> bool {
        self.state().find(id).is_none_or(|found| found.cancelled)
    }

    fn chose(self: &Arc<Self>, id: u64, index: usize) -> bool {
        let mut theirs = false;
        self.change(id, |ask| {
            theirs = index < ask.users.len();
            if theirs {
                ask.chosen = index;
                ask.question = None;
                ask.notice = None;
                ask.failures = 0;
            }
        });
        theirs
    }

    pub(crate) fn user(&self, id: u64) -> Option<User> {
        self.state()
            .find(id)
            .and_then(|found| found.ask.user().cloned())
    }

    pub(crate) fn running(&self, id: u64, interrupt: Stop) {
        let mut state = self.state();
        if let Some(found) = state.find_mut(id) {
            found.interrupt = Some(interrupt);
        }
    }

    pub(crate) fn asking(self: &Arc<Self>, id: u64, text: String, echo: bool) -> u64 {
        let serial = {
            let mut state = self.state();
            state.questions += 1;
            state.questions
        };
        self.change(id, |ask| {
            ask.question = Some(Question { serial, text, echo });
        });
        serial
    }

    pub(crate) fn working(self: &Arc<Self>, id: u64) {
        self.change(id, |ask| ask.question = None);
    }

    pub(crate) fn notify(self: &Arc<Self>, id: u64, notice: Notice) {
        self.change(id, |ask| ask.notice = Some(notice));
    }

    pub(crate) fn failed(self: &Arc<Self>, id: u64) -> u32 {
        let mut failures = 0;
        self.change(id, |ask| {
            ask.failures += 1;
            ask.question = None;
            failures = ask.failures;
        });
        failures
    }

    pub(crate) fn finish(self: &Arc<Self>, id: u64, outcome: Outcome) {
        let waiting = {
            let mut state = self.state();
            let at = state.queue.iter().position(|found| found.ask.id == id);
            at.and_then(|at| state.queue.remove(at))
                .and_then(|mut found| found.waiting.take())
        };
        if let Some(waiting) = waiting {
            let _ = waiting.send(outcome);
        }
        self.turn();
    }

    fn turn(self: &Arc<Self>) {
        let (start, asking) = {
            let mut state = self.state();
            let start = match state.queue.front_mut() {
                Some(front) if front.replies.is_none() && !front.cancelled => {
                    let (replies, incoming) = mpsc::channel();
                    front.replies = Some(replies);
                    Some((front.ask.id, front.cookie.clone(), incoming))
                }
                _ => None,
            };
            (start, state.asking())
        };
        if let Some((id, cookie, incoming)) = start {
            conversation::start(Arc::clone(self), id, cookie, incoming);
        }
        (self.report)(asking);
    }

    fn call_off(self: &Arc<Self>, matching: impl Fn(&Authentication) -> bool) {
        let ending: Vec<(Option<Sender<Reply>>, Option<Stop>)> = {
            let mut state = self.state();
            state
                .queue
                .iter_mut()
                .filter(|found| matching(found))
                .map(|found| {
                    found.cancelled = true;
                    (found.replies.clone(), found.interrupt.clone())
                })
                .collect()
        };
        if ending.is_empty() {
            return;
        }
        for (replies, interrupt) in ending {
            if let Some(replies) = replies {
                let _ = replies.send(Reply::Dismiss);
            }
            if let Some(interrupt) = interrupt {
                interrupt.stop();
            }
        }
        self.discard();
    }

    fn discard(self: &Arc<Self>) {
        let waiting: Vec<oneshot::Sender<Outcome>> = {
            let mut state = self.state();
            let mut waiting = Vec::new();
            state.queue.retain_mut(|found| {
                if !found.cancelled || found.replies.is_some() {
                    return true;
                }
                waiting.extend(found.waiting.take());
                false
            });
            waiting
        };
        for waiting in waiting {
            let _ = waiting.send(Outcome::Cancelled);
        }
        self.turn();
    }

    fn change(self: &Arc<Self>, id: u64, change: impl FnOnce(&mut Ask)) {
        let asking = {
            let mut state = self.state();
            let Some(found) = state.find_mut(id) else {
                return;
            };
            change(&mut found.ask);
            state.asking()
        };
        (self.report)(asking);
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock()
    }
}

impl State {
    fn find(&self, id: u64) -> Option<&Authentication> {
        self.queue.iter().find(|found| found.ask.id == id)
    }

    fn find_mut(&mut self, id: u64) -> Option<&mut Authentication> {
        self.queue.iter_mut().find(|found| found.ask.id == id)
    }

    fn asking(&self) -> Option<Box<Ask>> {
        self.queue.front().map(|front| Box::new(front.ask.clone()))
    }
}

impl std::fmt::Debug for Pending {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pending").finish_non_exhaustive()
    }
}

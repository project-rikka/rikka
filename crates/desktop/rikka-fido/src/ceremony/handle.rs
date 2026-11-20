use std::fmt;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use rikka_types::CancelToken;
use tokio::sync::{mpsc as events, oneshot};

use super::event::Event;
use super::pin::Pin;
use super::prompter::{Answer, Prompter};
use super::watch::{Events, Watch};
use crate::error::{Error, Result};

#[must_use = "dropping a ceremony cancels it"]
pub struct Ceremony<T> {
    events: events::UnboundedReceiver<Event>,
    answers: mpsc::Sender<Answer>,
    cancel: CancelToken,
    outcome: Option<oneshot::Receiver<Result<T>>>,
}

impl<T: Send + 'static> Ceremony<T> {
    pub(crate) fn start<F>(within: Duration, body: F) -> Result<Ceremony<T>>
    where
        F: FnOnce(&Watch, &Prompter) -> Result<T> + Send + 'static,
    {
        let (event_sender, event_receiver) = events::unbounded_channel();
        let (answer_sender, answer_receiver) = mpsc::channel();
        let (outcome_sender, outcome_receiver) = oneshot::channel();
        let cancel = CancelToken::new();
        let watch = Watch::new(
            cancel.clone(),
            Instant::now() + within,
            Events::new(event_sender),
        );

        thread::Builder::new()
            .name("rikka-fido".to_owned())
            .spawn(move || {
                let prompter = Prompter::new(answer_receiver);
                let outcome = body(&watch, &prompter);
                drop(watch);
                if outcome_sender.send(outcome).is_err() {
                    tracing::debug!("ceremony dropped before its outcome");
                }
            })
            .map_err(Error::Spawn)?;

        Ok(Ceremony {
            events: event_receiver,
            answers: answer_sender,
            cancel,
            outcome: Some(outcome_receiver),
        })
    }
}

impl<T> Ceremony<T> {
    pub async fn event(&mut self) -> Option<Event> {
        self.events.recv().await
    }

    pub fn blocking_event(&mut self) -> Option<Event> {
        self.events.blocking_recv()
    }

    pub fn try_event(&mut self) -> Option<Event> {
        self.events.try_recv().ok()
    }

    pub fn pin(&self, pin: impl Into<Pin>) {
        self.answer(Answer::Pin(pin.into()));
    }

    pub fn choose(&self, account: usize) {
        self.answer(Answer::Account(account));
    }

    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    fn answer(&self, answer: Answer) {
        if self.answers.send(answer).is_err() {
            tracing::debug!("ceremony finished before the answer arrived");
        }
    }

    #[must_use]
    pub fn cancel_token(&self) -> CancelToken {
        self.cancel.clone()
    }

    pub async fn finish(mut self) -> Result<T> {
        let Some(outcome) = self.outcome.as_mut() else {
            return Err(Error::Aborted);
        };
        let settled = outcome.await.unwrap_or(Err(Error::Aborted));
        self.outcome = None;
        settled
    }

    pub fn wait(mut self) -> Result<T> {
        match self.outcome.take() {
            Some(outcome) => outcome.blocking_recv().unwrap_or(Err(Error::Aborted)),
            None => Err(Error::Aborted),
        }
    }
}

impl<T> Drop for Ceremony<T> {
    fn drop(&mut self) {
        if self.outcome.is_some() {
            self.cancel.cancel();
        }
    }
}

impl<T> fmt::Debug for Ceremony<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Ceremony")
            .field("cancelled", &self.cancel.is_cancelled())
            .field("finished", &self.outcome.is_none())
            .finish_non_exhaustive()
    }
}

use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

use super::event::{Event, NewPinReason};
use super::pin::Pin;
use super::watch::Watch;
use crate::error::{Error, Result};
use crate::webauthn::User;

const LISTEN: Duration = Duration::from_millis(50);

#[derive(Debug)]
pub(crate) enum Answer {
    Pin(Pin),
    Account(usize),
}

#[derive(Debug)]
pub(crate) struct Prompter {
    answers: Receiver<Answer>,
}

impl Prompter {
    pub(crate) fn new(answers: Receiver<Answer>) -> Prompter {
        Prompter { answers }
    }

    pub(crate) fn pin(
        &self,
        watch: &Watch,
        retries: u32,
        min_length: usize,
        wrong: bool,
    ) -> Result<Pin> {
        self.ask(
            watch,
            Event::Pin {
                retries,
                min_length,
                wrong,
            },
            |answer| match answer {
                Answer::Pin(pin) => Some(pin),
                Answer::Account(_) => None,
            },
        )
    }

    pub(crate) fn new_pin(
        &self,
        watch: &Watch,
        min_length: usize,
        reason: NewPinReason,
        rejected: bool,
    ) -> Result<Pin> {
        self.ask(
            watch,
            Event::NewPin {
                min_length,
                reason,
                rejected,
            },
            |answer| match answer {
                Answer::Pin(pin) => Some(pin),
                Answer::Account(_) => None,
            },
        )
    }

    pub(crate) fn account(&self, watch: &Watch, users: Vec<User>) -> Result<usize> {
        let count = users.len();
        self.ask(watch, Event::Account { users }, |answer| match answer {
            Answer::Account(index) if index < count => Some(index),
            _ => None,
        })
    }

    fn ask<T>(
        &self,
        watch: &Watch,
        question: Event,
        accept: impl Fn(Answer) -> Option<T>,
    ) -> Result<T> {
        while self.answers.try_recv().is_ok() {}
        watch.emit(question);

        loop {
            watch.check()?;
            match self.answers.recv_timeout(LISTEN) {
                Ok(answer) => {
                    if let Some(accepted) = accept(answer) {
                        return Ok(accepted);
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Err(Error::Cancelled),
            }
        }
    }
}

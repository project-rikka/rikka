use std::collections::VecDeque;

use rikka_thread::{QuestionSet, Selection};

use crate::question_form::QuestionForm;

#[derive(Debug, Default)]
pub struct QuestionQueue {
    open: Option<QuestionForm>,
    behind: VecDeque<QuestionSet>,
}

impl QuestionQueue {
    pub fn open(&self) -> Option<&QuestionForm> {
        self.open.as_ref()
    }

    pub fn open_mut(&mut self) -> Option<&mut QuestionForm> {
        self.open.as_mut()
    }

    pub fn put(&mut self, survey: QuestionSet) {
        if self.open.is_none() {
            self.open = QuestionForm::new(survey);
            return;
        }
        self.behind.push_back(survey);
    }

    pub fn withdraw(&mut self, id: u64) {
        self.behind.retain(|behind| behind.id() != id);
        if self.open.as_ref().is_some_and(|open| open.id() == id) {
            self.open = None;
            self.next();
        }
    }

    pub fn track(&mut self, surveys: &[QuestionSet]) {
        let is_tracked = |id: u64| surveys.iter().any(|survey| survey.id() == id);
        self.behind.retain(|behind| is_tracked(behind.id()));
        if self
            .open
            .as_ref()
            .is_some_and(|open| !is_tracked(open.id()))
        {
            self.open = None;
        }
        for survey in surveys {
            let known = self
                .open
                .as_ref()
                .is_some_and(|open| open.id() == survey.id())
                || self.behind.iter().any(|behind| behind.id() == survey.id());
            if !known {
                self.behind.push_back(survey.clone());
            }
        }
        if self.open.is_none() {
            self.next();
        }
    }

    pub fn ahead(&mut self) -> Option<(u64, Vec<Selection>)> {
        if self.open.as_mut()?.ahead() {
            return None;
        }

        let question = self.open.take().expect("open question should exist");
        self.next();
        Some((question.id(), question.into_answers()))
    }

    fn next(&mut self) {
        while let Some(behind) = self.behind.pop_front() {
            self.open = QuestionForm::new(behind);
            if self.open.is_some() {
                return;
            }
        }
    }
}

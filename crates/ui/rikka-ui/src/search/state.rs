use std::cell::RefCell;
use std::rc::Rc;

use crate::search::found::Found;
use crate::search::hits::Tally;
use crate::search::query::{Options, Query, Rule};
use crate::text::TextDocument;
use crate::view::Observers;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Next,
    Previous,
    Replace,
    ReplaceAll,
}

#[derive(Clone, Debug)]
pub enum Action {
    Pattern,
    Replacement,
    Rule(Rule),
    Replacing(bool),
    Step(Step),
    Close,
}

#[derive(Debug, Default)]
struct Inner {
    open: bool,
    replacing: bool,
    pattern: TextDocument,
    replacement: TextDocument,
    options: Options,
    query: Query,
    tally: Tally,
    step: Option<Step>,
    looking: bool,
    open_count: u64,
    observers: Observers,
}

impl Inner {
    fn reread(&mut self) {
        self.query = Query::new(&self.pattern.text(), self.options);
        self.step = None;
    }
}

#[derive(Clone, Debug, Default)]
pub struct Search(Rc<RefCell<Inner>>);

impl Search {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        self.watched();
        self.0.borrow().open
    }

    #[must_use]
    pub fn is_replacing(&self) -> bool {
        self.watched();
        let state = self.0.borrow();
        state.open && state.replacing
    }

    pub fn open(&self, seed: Option<&str>) {
        self.open_with(false, seed);
    }

    pub fn open_replacing(&self, seed: Option<&str>) {
        self.open_with(true, seed);
    }

    fn open_with(&self, replacing: bool, seed: Option<&str>) {
        let mut state = self.0.borrow_mut();
        state.open = true;
        state.replacing = state.replacing || replacing;
        state.open_count += 1;
        if let Some(seed) = seed.filter(|seed| !seed.is_empty() && !seed.contains('\n')) {
            state.pattern.set_text(seed);
            state.pattern.select_all();
            state.reread();
        }
        drop(state);
        self.changed();
    }

    pub fn close(&self) {
        let mut state = self.0.borrow_mut();
        state.open = false;
        state.step = None;
        state.tally = Tally::default();
        state.looking = false;
        drop(state);
        self.changed();
    }

    pub fn act(&self, action: Action) {
        let mut state = self.0.borrow_mut();
        match action {
            Action::Pattern => state.reread(),
            Action::Replacement => {}
            Action::Rule(rule) => {
                state.options = state.options.toggled(rule);
                state.reread();
            }
            Action::Replacing(replacing) => state.replacing = replacing,
            Action::Step(step) => state.step = Some(step),
            Action::Close => {
                drop(state);
                self.close();
                return;
            }
        }
        drop(state);
        self.changed();
    }

    fn watched(&self) {
        self.0.borrow().observers.read();
    }

    fn changed(&self) {
        self.0.borrow().observers.changed();
    }

    #[must_use]
    pub fn take_step(&self) -> Option<Step> {
        self.watched();
        self.0.borrow_mut().step.take()
    }

    #[must_use]
    pub fn query(&self) -> Query {
        self.watched();
        self.0.borrow().query.clone()
    }

    #[must_use]
    pub fn found(&self) -> Found {
        self.watched();
        let state = self.0.borrow();
        match state.open {
            true => Found::new(state.query.clone()),
            false => Found::default(),
        }
    }

    #[must_use]
    pub fn options(&self) -> Options {
        self.watched();
        self.0.borrow().options
    }

    #[must_use]
    pub fn replacement(&self) -> String {
        self.watched();
        self.0.borrow().replacement.text()
    }

    pub fn seek(&self, pattern: &str) {
        let mut state = self.0.borrow_mut();
        state.pattern.set_text(pattern);
        state.reread();
        drop(state);
        self.changed();
    }

    pub fn set_replacement(&self, text: &str) {
        self.0.borrow().replacement.set_text(text);
        self.changed();
    }

    #[must_use]
    pub fn stamp(&self) -> u64 {
        self.watched();
        self.0.borrow().query.stamp()
    }

    #[must_use]
    pub fn opened_count(&self) -> u64 {
        self.watched();
        self.0.borrow().open_count
    }
    #[must_use]
    pub fn tally(&self) -> Tally {
        self.watched();
        self.0.borrow().tally
    }

    #[must_use]
    pub fn is_looking(&self) -> bool {
        self.watched();
        self.0.borrow().looking
    }

    pub fn report(&self, tally: Tally) -> bool {
        self.reported(tally, false)
    }

    pub fn reporting(&self, tally: Tally) -> bool {
        self.reported(tally, true)
    }

    fn reported(&self, tally: Tally, looking: bool) -> bool {
        let mut state = self.0.borrow_mut();
        if state.tally == tally && state.looking == looking {
            return false;
        }
        state.tally = tally;
        state.looking = looking;
        drop(state);
        self.changed();
        true
    }

    pub fn pattern_field(&self) -> TextDocument {
        self.0.borrow().pattern.clone()
    }

    pub fn replacement_field(&self) -> TextDocument {
        self.0.borrow().replacement.clone()
    }
}

use std::rc::Rc;

use ade_paths::{PathMatch, Paths};
use rikka_app::prelude::*;

pub(crate) const MOST: usize = 200;
const PAGE: isize = 10;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Choice {
    pub(crate) path_match: PathMatch,
    pub(crate) recent: bool,
}

#[derive(Debug)]
pub(crate) struct Palette {
    query: TextDocument,
    choices: Rc<[Choice]>,
    chosen: usize,
    request: u64,
}

impl Palette {
    pub(crate) fn new() -> Palette {
        Palette {
            query: TextDocument::new(""),
            choices: Rc::from([]),
            chosen: 0,
            request: 0,
        }
    }

    pub(crate) fn query(&self) -> &TextDocument {
        &self.query
    }

    pub(crate) fn choices(&self) -> Rc<[Choice]> {
        Rc::clone(&self.choices)
    }

    pub(crate) fn chosen(&self) -> usize {
        self.chosen
    }

    pub(crate) fn ask(&mut self) -> (u64, String) {
        self.request += 1;
        (self.request, self.query.text())
    }

    pub(crate) fn answered(&mut self, request: u64, choices: Vec<Choice>) -> bool {
        if request != self.request {
            return false;
        }
        self.choices = choices.into();
        self.chosen = 0;
        true
    }

    pub(crate) fn step(&mut self, by: isize) {
        let count = self.choices.len() as isize;
        if count == 0 {
            return;
        }
        self.chosen = match by.unsigned_abs() > 1 {
            true => (self.chosen as isize + by).clamp(0, count - 1) as usize,
            false => (self.chosen as isize + by).rem_euclid(count) as usize,
        };
    }

    pub(crate) fn page(&mut self, down: bool) {
        self.step(if down { PAGE } else { -PAGE });
    }

    pub(crate) fn choose(&mut self, at: usize) -> Option<&Choice> {
        self.chosen = at;
        self.choices.get(at)
    }
}

pub(crate) fn choices(paths: &Paths, recent: Vec<String>, query: &str) -> Vec<Choice> {
    let remembered = Paths::listed(recent.clone()).files(query, MOST);
    let rest = paths
        .files(query, MOST)
        .into_iter()
        .filter(|path_match| !recent.iter().any(|recent| recent == path_match.path()));
    remembered
        .into_iter()
        .map(|path_match| Choice {
            path_match,
            recent: true,
        })
        .chain(rest.map(|path_match| Choice {
            path_match,
            recent: false,
        }))
        .take(MOST)
        .collect()
}

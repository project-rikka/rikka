use browser_preferences::CustomEngine;
use rikka_app::prelude::*;
use rikka_omnibox::{Error, SearchEngines};

use crate::search::engine_of;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Editing {
    New,
    Existing(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Problem {
    Name,
    Keyword,
    KeywordTaken,
    Address,
}

#[derive(State)]
pub(crate) struct EngineForm {
    #[untracked]
    pub(crate) name: TextDocument,
    #[untracked]
    pub(crate) keyword: TextDocument,
    #[untracked]
    pub(crate) url: TextDocument,
    #[untracked]
    pub(crate) suggestions: TextDocument,
    editing: Tracked<Option<Editing>>,
    problem: Tracked<Option<Problem>>,
}

impl EngineForm {
    pub(crate) fn new() -> EngineForm {
        EngineForm {
            name: TextDocument::new(""),
            keyword: TextDocument::new(""),
            url: TextDocument::new(""),
            suggestions: TextDocument::new(""),
            editing: Tracked::new(None),
            problem: Tracked::new(None),
        }
    }

    pub(crate) fn editing(&self) -> Option<&Editing> {
        self.editing.as_ref()
    }

    pub(crate) fn problem(&self) -> Option<Problem> {
        *self.problem
    }

    pub(crate) fn begin(&mut self, existing: Option<&CustomEngine>) {
        let blank = CustomEngine::default();
        let shown = existing.unwrap_or(&blank);
        self.name.reset(&shown.name);
        self.keyword.reset(&shown.keyword);
        self.url.reset(&shown.url);
        self.suggestions.reset(&shown.suggestions);
        self.problem.set(None);
        self.editing.set(Some(match existing {
            Some(existing) => Editing::Existing(existing.id.clone()),
            None => Editing::New,
        }));
    }

    pub(crate) fn cancel(&mut self) {
        self.editing.set(None);
        self.problem.set(None);
    }

    pub(crate) fn finish(
        &mut self,
        custom_engines: &[CustomEngine],
        engines: &SearchEngines,
    ) -> Option<Vec<CustomEngine>> {
        let editing = self.editing.as_ref()?.clone();
        let id = match &editing {
            Editing::Existing(id) => id.clone(),
            Editing::New => fresh_id(custom_engines),
        };
        let engine = CustomEngine {
            id: id.clone(),
            name: self.name.text().trim().to_owned(),
            keyword: self.keyword.text().trim().to_owned(),
            url: self.url.text().trim().to_owned(),
            suggestions: self.suggestions.text().trim().to_owned(),
        };
        match checked(&engine, engines) {
            Ok(()) => {
                self.cancel();
                let mut next: Vec<CustomEngine> = custom_engines.to_vec();
                match next.iter_mut().find(|candidate| candidate.id == id) {
                    Some(existing_engine) => *existing_engine = engine,
                    None => next.push(engine),
                }
                Some(next)
            }
            Err(problem) => {
                self.problem.set(Some(problem));
                None
            }
        }
    }
}

impl std::fmt::Debug for EngineForm {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EngineForm")
            .field("editing", &*self.editing)
            .finish_non_exhaustive()
    }
}

fn fresh_id(custom_engines: &[CustomEngine]) -> String {
    let next = custom_engines
        .iter()
        .filter_map(|engine| engine.id.strip_prefix("custom-")?.parse::<u64>().ok())
        .max()
        .map_or(1, |highest| highest + 1);
    format!("custom-{next}")
}

fn checked(engine: &CustomEngine, engines: &SearchEngines) -> Result<(), Problem> {
    let built = engine_of(engine).map_err(problem_of)?;
    let mut others = engines.clone();
    let _ = others.remove(&engine.id);
    others.add(built).map_err(problem_of)
}

fn problem_of(error: Error) -> Problem {
    match error {
        Error::MissingName => Problem::Name,
        Error::InvalidKeyword(_) => Problem::Keyword,
        Error::DuplicateKeyword(_) => Problem::KeywordTaken,
        _ => Problem::Address,
    }
}

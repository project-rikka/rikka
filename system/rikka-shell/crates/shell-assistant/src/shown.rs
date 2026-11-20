use std::sync::Arc;

use rikka_llm::{ModelInfo, Trouble};
use rikka_llm_accounts::Selection;
use rikka_thread::{Answer, Job, QuestionSet, Summary, Thread};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Readiness {
    #[default]
    Opening,
    Ready,
    Unready,
}

#[derive(Clone, Debug)]
pub(crate) struct Shown {
    pub(crate) readiness: Readiness,
    pub(crate) thread: Arc<Thread>,
    pub(crate) answering: Option<Answer>,
    pub(super) summaries: Arc<[Summary]>,
    pub(crate) surveys: Vec<QuestionSet>,
    pub(crate) trouble: Option<Trouble>,
    pub(crate) jobs: Vec<Job>,
    pub(crate) selection: Option<Selection>,
    pub(crate) model: Option<ModelInfo>,
}

impl Shown {
    pub(super) fn is_answering(&self) -> bool {
        self.answering.is_some()
    }

    pub(crate) fn is_ready(&self) -> bool {
        self.readiness == Readiness::Ready
    }
}

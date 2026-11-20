use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use rikka_llm::{Content, Message, Usage};
use rikka_llm_accounts::{ConnectedModel, Llm, Selection};

use crate::ask::Ask;
use crate::attachment::Attachment;
use crate::replaying::{self, ResidentBodies};
use crate::round::Round;
use crate::turn::{Body, Speaker, Turn, TurnText, asked_amid};

const TITLE: usize = 60;

static MINTED: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Summary {
    id: String,
    #[serde(default)]
    title: String,
    created: u64,
    updated: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    worktree: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    selection: Option<Selection>,
    #[serde(default)]
    filled: u64,
    #[serde(default)]
    turns: usize,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    model: String,
}

impl Summary {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub fn updated(&self) -> u64 {
        self.updated
    }

    #[must_use]
    pub fn worktree(&self) -> Option<&Path> {
        self.worktree.as_deref()
    }

    #[must_use]
    pub fn selection(&self) -> Option<&Selection> {
        self.selection.as_ref()
    }

    #[must_use]
    pub fn selection_in(&self, llm: &Llm) -> Option<Selection> {
        self.selection
            .clone()
            .or_else(|| llm.restore_selection(&self.model))
    }

    #[must_use]
    pub fn context_tokens(&self) -> u64 {
        self.filled
    }

    #[must_use]
    pub fn turns(&self) -> usize {
        self.turns
    }

    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Thread {
    id: String,
    title: String,
    created: u64,
    updated: u64,
    worktree: Option<PathBuf>,
    selection: Option<Selection>,
    turns: Vec<Arc<Turn>>,
    filled: u64,
    running: bool,
}

impl Thread {
    #[must_use]
    pub fn new() -> Thread {
        let at = now();
        Thread {
            id: format!(
                "{at:013}-{:04}",
                MINTED.fetch_add(1, Ordering::Relaxed) % 10_000
            ),
            title: String::new(),
            created: at,
            updated: at,
            worktree: None,
            selection: None,
            turns: Vec::new(),
            filled: 0,
            running: false,
        }
    }

    pub(crate) fn with_id(id: String) -> Thread {
        Thread {
            id,
            ..Thread::new()
        }
    }

    #[must_use]
    pub fn selection(&self) -> Option<&Selection> {
        self.selection.as_ref()
    }

    #[must_use]
    pub fn selection_in(&self, llm: &Llm) -> Option<Selection> {
        self.selection
            .clone()
            .or_else(|| llm.restore_selection(self.last_model()))
    }

    pub fn connect_model(&mut self, llm: &Llm) -> rikka_llm_accounts::Result<ConnectedModel> {
        let selection = self.selection_in(llm);
        let model = llm.roster().connect(selection.as_ref())?;
        if let Some(selection) = selection {
            self.select(selection);
        }
        Ok(model)
    }

    pub fn select(&mut self, selection: Selection) {
        self.selection = Some(selection);
    }

    pub fn set_worktree(&mut self, worktree: Option<PathBuf>) {
        self.worktree = worktree;
    }

    #[must_use]
    pub fn worktree(&self) -> Option<&Path> {
        self.worktree.as_deref()
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub fn updated(&self) -> u64 {
        self.updated
    }

    #[must_use]
    pub fn turns(&self) -> &[Arc<Turn>] {
        match self.running {
            true => self.turns.split_last().map_or(&[], |(_, earlier)| earlier),
            false => &self.turns,
        }
    }

    #[must_use]
    pub fn all_turns(&self) -> &[Arc<Turn>] {
        &self.turns
    }

    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running
    }

    #[must_use]
    pub fn summary(&self) -> Summary {
        Summary {
            id: self.id.clone(),
            title: self.title.clone(),
            created: self.created,
            updated: self.updated,
            worktree: self.worktree.clone(),
            selection: self.selection.clone(),
            filled: self.filled,
            turns: self.turns().len(),
            model: self.last_model().to_owned(),
        }
    }

    fn last_model(&self) -> &str {
        self.turns
            .iter()
            .rev()
            .map(|turn| turn.model())
            .find(|model| !model.is_empty())
            .unwrap_or_default()
    }

    #[must_use]
    pub fn from_turns(summary: Summary, turns: Vec<Arc<Turn>>) -> Thread {
        Thread {
            id: summary.id,
            title: summary.title,
            created: summary.created,
            updated: summary.updated,
            worktree: summary.worktree,
            selection: summary.selection,
            turns,
            filled: summary.filled,
            running: false,
        }
    }

    #[must_use]
    pub fn context_tokens(&self) -> u64 {
        self.filled
    }

    pub fn record_usage(&mut self, usage: Usage) {
        self.filled = usage.total();
    }

    pub fn push_ask(&mut self, ask: impl Into<Ask>) {
        self.add_request(Speaker::User, ask.into(), None);
    }

    pub(crate) fn add_request(
        &mut self,
        speaker: Speaker,
        ask: Ask,
        instructions: Option<Vec<String>>,
    ) -> u64 {
        self.asked_at(speaker, ask, instructions, now())
    }

    pub(crate) fn asked_at(
        &mut self,
        speaker: Speaker,
        ask: Ask,
        instructions: Option<Vec<String>>,
        at: u64,
    ) -> u64 {
        let (text, attachments) = ask.into_turn_text(instructions);
        if speaker == Speaker::User && self.title.is_empty() {
            self.title = title_of(text.text(), &attachments);
        }
        self.add_at(speaker, text, attachments, Vec::new(), at);
        at
    }

    pub(crate) fn last_asked(&self) -> Option<u64> {
        self.turns
            .iter()
            .rev()
            .find(|turn| matches!(turn.speaker(), Speaker::User | Speaker::Notice))
            .map(|turn| turn.at())
    }

    pub fn rewind(&mut self, stamp: u64) -> bool {
        if self.running {
            return false;
        }
        let Some(from) = self.turns.iter().position(|turn| turn.stamp() == stamp) else {
            return false;
        };
        self.turns.truncate(from);
        if !self
            .turns
            .iter()
            .any(|turn| turn.speaker() == Speaker::User)
        {
            self.title.clear();
        }
        self.filled = 0;
        self.updated = now();
        true
    }

    pub fn answer(&mut self, text: impl Into<TurnText>, rounds: Vec<Round>) {
        self.set_answer(text.into(), rounds, false);
    }

    pub fn answer_partial(&mut self, text: impl Into<TurnText>, rounds: Vec<Round>) {
        self.set_answer(text.into(), rounds, true);
    }

    fn set_answer(&mut self, text: TurnText, rounds: Vec<Round>, running: bool) {
        if text.text.trim().is_empty() && rounds.is_empty() {
            self.shed();
            return;
        }
        self.add(Speaker::Model, text, Vec::new(), rounds);
        self.running = running;
    }

    pub fn compact(&mut self, summary: impl Into<TurnText>) {
        self.add(Speaker::Summary, summary.into(), Vec::new(), Vec::new());
        self.filled = 0;
    }

    pub(crate) fn instructed(&self) -> Option<(Arc<Body>, Arc<Body>)> {
        let first = self.turns.iter().position(|turn| turn.is_instructed())?;
        let latest = self
            .turns
            .iter()
            .rposition(|turn| turn.is_instructed())
            .filter(|latest| *latest >= self.folded())
            .unwrap_or(first);
        let sent = self.turns[first].body();
        let conveyed = match latest == first {
            true => Arc::clone(&sent),
            false => self.turns[latest].body(),
        };
        Some((sent, conveyed))
    }

    #[must_use]
    pub fn transcript(&self, model: &str) -> Vec<Message> {
        let folded = self.folded();
        let instructed = self.turns.iter().position(|turn| turn.is_instructed());
        let mut transcript = Vec::with_capacity(self.turns.len() - folded);
        let mut summary: Option<String> = None;
        let bodies: Vec<Arc<Body>> = self.turns[folded..]
            .iter()
            .map(|turn| turn.body())
            .collect();
        let resident = ResidentBodies::new(&bodies);

        for ((at, turn), body) in self.turns.iter().enumerate().skip(folded).zip(&bodies) {
            match turn.speaker() {
                Speaker::Summary => summary = Some(body.text().to_owned()),
                Speaker::User => {
                    let text = asked_amid(body.context(), body.sent_text(), body.attachments());
                    transcript.push(Message::user(led(summary.take(), text)));
                }
                Speaker::Notice => {
                    let notice = asked_amid(body.context(), body.sent_text(), &[]);
                    transcript.push(Message::user(led(summary.take(), notice)));
                }
                Speaker::Model => {
                    if let Some(standing) = summary.take() {
                        transcript.push(Message::user(standing));
                    }
                    let again = turn.model() == model && instructed.is_some_and(|first| first < at);
                    replaying::model_turn(&mut transcript, body, again, &resident);
                }
            }
        }

        if let Some(standing) = summary {
            transcript.push(Message::user(standing));
        }

        transcript
    }

    fn folded(&self) -> usize {
        self.turns
            .iter()
            .rposition(|turn| turn.speaker() == Speaker::Summary)
            .unwrap_or(0)
    }

    fn add(
        &mut self,
        speaker: Speaker,
        text: TurnText,
        attachments: Vec<Attachment>,
        rounds: Vec<Round>,
    ) {
        self.add_at(speaker, text, attachments, rounds, now());
    }

    fn add_at(
        &mut self,
        speaker: Speaker,
        text: TurnText,
        attachments: Vec<Attachment>,
        rounds: Vec<Round>,
        at: u64,
    ) {
        self.shed();
        self.turns
            .push(Arc::new(Turn::new(speaker, text, attachments, rounds, at)));
        self.updated = self.updated.max(at);
    }

    fn shed(&mut self) {
        if std::mem::take(&mut self.running) {
            self.turns.pop();
        }
    }
}

impl Default for Thread {
    fn default() -> Thread {
        Thread::new()
    }
}

fn led(summary: Option<String>, content: Vec<Content>) -> Vec<Content> {
    match summary {
        Some(summary) => std::iter::once(Content::text(summary))
            .chain(content)
            .collect(),
        None => content,
    }
}

fn title_of(text: &str, attachments: &[Attachment]) -> String {
    let typed = text.trim().lines().next().unwrap_or("").trim();
    let line = match typed.is_empty() {
        true => attachments.first().map(Attachment::name).unwrap_or(""),
        false => typed,
    };
    rikka_format::truncate(line, TITLE).into_owned()
}

pub(crate) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

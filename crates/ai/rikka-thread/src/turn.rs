use rikka_sync::{Mutex, MutexGuard};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};

use rikka_llm::Content;

use crate::attachment::Attachment;
use crate::journal::{Journal, Placement};
use crate::replay::Replay;
use crate::resident;
use crate::round::Round;

static STAMPED: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Speaker {
    #[serde(rename = "you")]
    User,
    Model,
    Notice,
    Summary,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TurnText {
    pub(crate) text: String,
    pub(crate) expanded: Option<String>,
    pub(crate) replay: Option<Replay>,
    pub(crate) model: String,
    pub(crate) context: Vec<String>,
    pub(crate) instructions: Option<Vec<String>>,
}

impl TurnText {
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

impl<T: Into<String>> From<T> for TurnText {
    fn from(text: T) -> TurnText {
        TurnText {
            text: text.into(),
            ..TurnText::default()
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Body {
    text: String,
    expanded: Option<String>,
    model: String,
    replay: Option<Replay>,
    context: Vec<String>,
    instructions: Option<Vec<String>>,
    attachments: Vec<Attachment>,
    rounds: Vec<Round>,
}

impl Body {
    pub(crate) fn new(reply: TurnText, attachments: Vec<Attachment>, rounds: Vec<Round>) -> Body {
        Body {
            text: reply.text,
            expanded: reply.expanded,
            model: reply.model,
            replay: reply.replay,
            context: reply.context,
            instructions: reply.instructions,
            attachments,
            rounds,
        }
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn sent_text(&self) -> &str {
        self.expanded.as_deref().unwrap_or(&self.text)
    }

    pub(crate) fn expanded(&self) -> Option<&str> {
        self.expanded.as_deref()
    }

    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }

    pub(crate) fn replay(&self) -> Option<&Replay> {
        self.replay.as_ref()
    }

    #[must_use]
    pub fn context(&self) -> &[String] {
        &self.context
    }

    #[must_use]
    pub fn attachments(&self) -> &[Attachment] {
        &self.attachments
    }

    #[must_use]
    pub fn rounds(&self) -> &[Round] {
        &self.rounds
    }

    pub(crate) fn instructions(&self) -> Option<&[String]> {
        self.instructions.as_deref()
    }

    pub(crate) fn bytes(&self) -> usize {
        let replay = self.replay.as_ref().map_or(0, Replay::bytes);
        let context: usize = self.context.iter().map(String::len).sum();
        let instructions: usize = self.instructions.iter().flatten().map(String::len).sum();
        let attached: usize = self.attachments.iter().map(|it| it.data().len()).sum();
        let rounds: usize = self.rounds.iter().map(Round::bytes).sum();
        let length = self.text.len() + self.expanded.as_ref().map_or(0, String::len);
        length + self.model.len() + replay + context + instructions + attached + rounds
    }

    fn brief(&self) -> Brief {
        Brief::new(
            !self.text.trim().is_empty(),
            &self.model,
            &self.rounds,
            self.instructions.is_some(),
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Brief {
    has_text: bool,
    model: Arc<str>,
    rounds: Vec<RoundSummary>,
    instructed: bool,
}

impl Brief {
    pub(crate) fn new(has_text: bool, model: &str, rounds: &[Round], instructed: bool) -> Brief {
        Brief {
            has_text,
            model: Arc::from(model),
            rounds: rounds.iter().map(RoundSummary::new).collect(),
            instructed,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RoundSummary {
    has_text: bool,
    calls: usize,
    notices: usize,
}

impl RoundSummary {
    #[must_use]
    pub fn new(round: &Round) -> RoundSummary {
        RoundSummary {
            has_text: !round.text().trim().is_empty(),
            calls: round.calls().len(),
            notices: round.notices().len(),
        }
    }

    #[must_use]
    pub fn has_text(&self) -> bool {
        self.has_text
    }

    #[must_use]
    pub fn calls(&self) -> usize {
        self.calls
    }

    #[must_use]
    pub fn notices(&self) -> usize {
        self.notices
    }
}

pub(crate) fn asked_amid(
    context: &[String],
    text: &str,
    attachments: &[Attachment],
) -> Vec<Content> {
    let text_content = (!text.trim().is_empty()).then(|| Content::text(text));
    context
        .iter()
        .map(|text| Content::text(text.as_str()))
        .chain(attachments.iter().map(Attachment::content))
        .chain(text_content)
        .collect()
}

#[derive(Debug)]
pub struct Turn {
    speaker: Speaker,
    at: u64,
    stamp: u64,
    brief: Brief,
    holding: Mutex<Holding>,
}

#[derive(Debug)]
enum Holding {
    Here(Arc<Body>),
    Away {
        journal: Arc<Journal>,
        placement: Placement,
        last: Weak<Body>,
    },
}

impl PartialEq for Turn {
    fn eq(&self, other: &Turn) -> bool {
        self.speaker == other.speaker
            && self.at == other.at
            && self.brief == other.brief
            && *self.body() == *other.body()
    }
}

impl Eq for Turn {}

impl Turn {
    pub(crate) fn new(
        speaker: Speaker,
        text: TurnText,
        attachments: Vec<Attachment>,
        rounds: Vec<Round>,
        at: u64,
    ) -> Turn {
        let body = Arc::new(Body::new(text, attachments, rounds));
        Turn {
            speaker,
            at,
            stamp: STAMPED.fetch_add(1, Ordering::Relaxed),
            brief: body.brief(),
            holding: Mutex::new(Holding::Here(body)),
        }
    }

    pub(crate) fn read(
        speaker: Speaker,
        at: u64,
        brief: Brief,
        journal: &Arc<Journal>,
        placement: Placement,
    ) -> Turn {
        Turn {
            speaker,
            at,
            stamp: STAMPED.fetch_add(1, Ordering::Relaxed),
            brief,
            holding: Mutex::new(Holding::Away {
                journal: Arc::clone(journal),
                placement,
                last: Weak::new(),
            }),
        }
    }

    #[must_use]
    pub fn stamp(&self) -> u64 {
        self.stamp
    }

    #[must_use]
    pub fn speaker(&self) -> Speaker {
        self.speaker
    }

    #[must_use]
    pub fn at(&self) -> u64 {
        self.at
    }

    #[must_use]
    pub fn model(&self) -> &str {
        &self.brief.model
    }

    #[must_use]
    pub fn spoke(&self) -> bool {
        self.brief.has_text
    }

    #[must_use]
    pub fn round_summaries(&self) -> &[RoundSummary] {
        &self.brief.rounds
    }

    pub(crate) fn is_instructed(&self) -> bool {
        self.brief.instructed
    }

    #[must_use]
    pub fn body(&self) -> Arc<Body> {
        let mut holding = self.lock_holding();
        match &mut *holding {
            Holding::Here(body) => Arc::clone(body),
            Holding::Away {
                journal,
                placement,
                last,
            } => {
                if let Some(body) = last.upgrade() {
                    resident::keep(&body);
                    return body;
                }
                let Some((read, found)) = journal.read(*placement, self.speaker, self.at) else {
                    tracing::error!("turn missing from thread journal");
                    return Arc::new(Body::default());
                };
                *placement = found;
                let body = Arc::new(read);
                *last = Arc::downgrade(&body);
                resident::keep(&body);
                body
            }
        }
    }

    pub(crate) fn settled_at(&self, journal: &Arc<Journal>, placement: Placement) {
        let mut holding = self.lock_holding();
        let last = match &*holding {
            Holding::Here(body) => {
                resident::keep(body);
                Arc::downgrade(body)
            }
            Holding::Away { last, .. } => last.clone(),
        };
        *holding = Holding::Away {
            journal: Arc::clone(journal),
            placement,
            last,
        };
    }

    pub(crate) fn written_at(&self) -> Option<(Arc<Journal>, Placement)> {
        match &*self.lock_holding() {
            Holding::Here(_) => None,
            Holding::Away {
                journal, placement, ..
            } => Some((Arc::clone(journal), *placement)),
        }
    }

    fn lock_holding(&self) -> MutexGuard<'_, Holding> {
        self.holding.lock()
    }
}

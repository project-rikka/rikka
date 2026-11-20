use rikka_sync::Mutex;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use rikka_llm::Effort;
use rikka_tools::State;
use serde::{Deserialize, Serialize};

use super::format::META;
use super::writer::{Entry, WriterState};
use crate::error::Result;
use crate::thread::Thread;

const AGENTS: &str = "agents";
const MEMBER: &str = "agent.json";

#[derive(Clone, Debug)]
pub struct Crew {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    writer: Arc<WriterState>,
    thread: String,
    kept: Mutex<Option<BTreeMap<u64, Kept>>>,
}

#[derive(Debug)]
struct Kept {
    member: Member,
    transcript: Option<Arc<Thread>>,
    here: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Member {
    pub(crate) job: u64,
    pub(crate) label: String,
    pub(crate) model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) effort: Option<Effort>,
    pub(crate) standing: Standing,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) report: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Standing {
    Running,
    Reported,
    Failed(String),
    Stopped,
    Interrupted,
    Closed,
}

impl Standing {
    pub(crate) fn of(state: &State) -> Standing {
        match state {
            State::Running => Standing::Running,
            State::Reported => Standing::Reported,
            State::Failed(reason) => Standing::Failed(reason.clone()),
            State::Stopped => Standing::Stopped,
            State::Interrupted => Standing::Interrupted,
            State::Closed => Standing::Closed,
            State::Exited(code) => Standing::Failed(format!("exited {code}")),
            State::Signalled => Standing::Failed("killed by a signal".to_owned()),
        }
    }

    pub(crate) fn state(&self) -> State {
        match self {
            Standing::Running | Standing::Closed => State::Closed,
            Standing::Reported => State::Reported,
            Standing::Failed(reason) => State::Failed(reason.clone()),
            Standing::Stopped => State::Stopped,
            Standing::Interrupted => State::Interrupted,
        }
    }
}

impl Crew {
    pub(super) fn new(writer: Arc<WriterState>, thread: &str) -> Crew {
        Crew {
            inner: Arc::new(Inner {
                writer,
                thread: thread.to_owned(),
                kept: Mutex::new(None),
            }),
        }
    }

    pub(crate) fn recalled(&self) -> Vec<Member> {
        self.with_kept(|kept| {
            kept.values()
                .filter(|kept| !(kept.here && kept.member.standing == Standing::Running))
                .map(|kept| kept.member.clone())
                .collect()
        })
    }

    pub(crate) fn keep(&self, member: Member) {
        let encoded = match serde_json::to_vec_pretty(&member) {
            Ok(encoded) => encoded,
            Err(error) => {
                tracing::error!("failed to encode agent {}: {error}", member.job);
                return;
            }
        };
        let at = format!("{}/{MEMBER}", self.transcript_id(member.job));
        self.with_kept(|kept| match kept.get_mut(&member.job) {
            Some(known) => {
                known.member = member;
                known.here = true;
            }
            None => {
                kept.insert(
                    member.job,
                    Kept {
                        member,
                        transcript: None,
                        here: true,
                    },
                );
            }
        });
        self.inner.writer.put(&at, Entry::Document(encoded));
    }

    pub(crate) fn keep_transcript(&self, job: u64, transcript: &Arc<Thread>) {
        self.with_kept(|kept| {
            if let Some(known) = kept.get_mut(&job) {
                known.transcript = Some(Arc::clone(transcript));
            }
        });
        self.inner
            .writer
            .put(transcript.id(), Entry::Thread(Thread::clone(transcript)));
    }

    pub(crate) fn transcript(&self, job: u64) -> Result<Option<Thread>> {
        let id = self.transcript_id(job);
        let folder = &self.inner.writer.folder;
        self.with_kept(|kept| {
            let Some(known) = kept.get_mut(&job) else {
                return Ok(None);
            };
            if let Some(transcript) = &known.transcript {
                return Ok(Some(Thread::clone(transcript)));
            }
            let read = match folder.join(&id).join(META).exists() {
                true => self.inner.writer.read(&id)?,
                false => Thread::with_id(id),
            };
            known.transcript = Some(Arc::new(read.clone()));
            Ok(Some(read))
        })
    }

    pub(crate) fn transcript_id(&self, job: u64) -> String {
        format!("{}/{AGENTS}/{job}", self.inner.thread)
    }

    fn with_kept<R>(&self, act: impl FnOnce(&mut BTreeMap<u64, Kept>) -> R) -> R {
        let mut kept = self.inner.kept.lock();
        let kept = kept.get_or_insert_with(|| {
            read_members(
                &self
                    .inner
                    .writer
                    .folder
                    .join(&self.inner.thread)
                    .join(AGENTS),
            )
        });
        act(kept)
    }
}

fn read_members(folder: &Path) -> BTreeMap<u64, Kept> {
    let listing = match std::fs::read_dir(folder) {
        Ok(listing) => listing,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return BTreeMap::new(),
        Err(error) => {
            tracing::warn!("failed to read {}: {error}", folder.display());
            return BTreeMap::new();
        }
    };
    listing
        .filter_map(|entry| {
            let at = entry.ok()?.path().join(MEMBER);
            match read_member(&at) {
                Ok(member) => Some((
                    member.job,
                    Kept {
                        member,
                        transcript: None,
                        here: false,
                    },
                )),
                Err(error) => {
                    tracing::warn!("{}: {error}", at.display());
                    None
                }
            }
        })
        .collect()
}

fn read_member(at: &Path) -> std::result::Result<Member, String> {
    let text = std::fs::read_to_string(at).map_err(|error| error.to_string())?;
    serde_json::from_str(&text).map_err(|error| error.to_string())
}

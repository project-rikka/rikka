use std::path::{Path, PathBuf};
use std::sync::Arc;

use rikka_app::prelude::*;
use rikka_chat::{Attached, Gathered, gather, kept_attachments, mentioned, needs_mcp};
use rikka_thread::{Ask, Attachment, History, RewindReport, Speaker, Turn};

use crate::project::act::{given, given_with_setup};
use crate::project::attaching;
use crate::project::catalogs::Catalogs;
use crate::project::chat::AdeChat;
use crate::setup::Setup;

use super::Project;

struct Resending {
    thread: String,
    said: String,
    kept: Vec<Attachment>,
    restoring: Option<(History, u64)>,
}

struct Resent {
    thread: String,
    ask: Ask,
    attached: Vec<Attachment>,
    unsent: Option<String>,
    refused: Option<String>,
    rewound: Option<Result<RewindReport, String>>,
    root: PathBuf,
    catalogs: Catalogs,
}

impl Project {
    pub(super) fn is_amendable(&self) -> bool {
        !*self.sending
            && self.watching().is_none()
            && self
                .runner()
                .is_none_or(|runner| !runner.is_busy() && runner.asking().is_none())
    }

    pub(in crate::project) fn amend(&mut self, turn: u64) {
        if !self.is_amendable() {
            return;
        }
        let Some(sent) = self.sent_turn(turn) else {
            return;
        };
        self.put_away();
        self.transcript.amend(turn, sent.body().text());

        let Some(history) = self.history() else {
            self.transcript.amend_touched(turn, Vec::new());
            return;
        };
        let root = self.selected_root();
        let since = sent.at();
        spawn_blocking(
            move || changed_files(&history, since, &root),
            given(move |project, changed: Vec<String>| {
                project.transcript.amend_touched(turn, changed);
            }),
        );
    }

    pub(in crate::project) fn amended(&mut self) {
        let Some(amending) = self.transcript.amending().cloned() else {
            return;
        };
        if !self.is_amendable() {
            return;
        }
        let turn = amending.stamp();
        let Some(sent) = self.sent_turn(turn) else {
            self.transcript.stop_amending();
            return;
        };

        let body = sent.body();
        let kept = kept_attachments(body.text(), body.sent_text(), body.attachments());
        let text = amending.text().trim().to_owned();
        if text.is_empty() && kept.is_empty() {
            return;
        }

        let ask = self.expanding(text.clone());
        let restoring = match amending.restores() {
            true => self.history().map(|history| (history, sent.at())),
            false => None,
        };
        self.transcript.stop_amending();
        let thread = self.open.to_string();
        let cut = self
            .thread_mut_of(&thread)
            .is_some_and(|rewound| rewound.rewind(turn));
        if !cut {
            return;
        }
        self.keep(&thread);

        self.sending.set(true);
        let resending = Resending {
            thread,
            said: text,
            kept,
            restoring,
        };
        if !needs_mcp(&self.hub, &ask) {
            let mentions = mentioned(ask.sent_text());
            self.resend(
                resending,
                Ok(Gathered {
                    ask,
                    attached: Attached::default(),
                    mentions,
                }),
            );
            return;
        }
        let hub = self.hub.clone();
        spawn(
            async move {
                let gathered = gather::<AdeChat>(&hub, ask).await;
                (resending, gathered)
            },
            given(
                |project, (resending, gathered): (Resending, Result<Gathered, String>)| {
                    project.resend(resending, gathered);
                },
            ),
        );
    }

    fn resend(&mut self, resending: Resending, gathered: Result<Gathered, String>) {
        let root = self.selected_root();
        spawn_blocking(
            move || {
                let Resending {
                    thread,
                    said,
                    kept,
                    restoring,
                } = resending;
                let rewound = restoring.map(|(history, since)| {
                    history.rewind(since).map_err(|error| error.to_string())
                });
                let (ask, read, failed) = match gathered {
                    Ok(Gathered {
                        ask,
                        attached,
                        mentions,
                    }) => {
                        let read = match mentions.is_empty() {
                            true => attached,
                            false => attached.join(attaching::mentioned(&root, &mentions)),
                        };
                        (ask, read, None)
                    }
                    Err(reason) => (Ask::new(said), Attached::default(), Some(reason)),
                };
                let mut attached = kept;
                attached.extend(read.attachments);
                Resent {
                    thread,
                    ask,
                    attached,
                    unsent: failed,
                    refused: read.refused,
                    rewound,
                    catalogs: Catalogs::find(&root),
                    root,
                }
            },
            given_with_setup(|project, setup, resent: Resent| project.resent(resent, setup)),
        );
    }

    fn resent(&mut self, resent: Resent, setup: &mut Setup) {
        self.sending.set(false);
        let Resent {
            thread,
            ask,
            attached,
            unsent,
            refused,
            rewound,
            root,
            catalogs,
        } = resent;

        self.catalogued(&root, catalogs, setup);
        let ask = ask.with_attachments(attached);
        if *self.open != thread || unsent.is_some() {
            self.redraft_in(&thread, vec![ask]);
        } else if let Err(start_failed) = self.converse(ask.clone(), setup) {
            self.redraft(vec![ask]);
            self.start_failed(start_failed);
        } else {
            self.error_message.set(None);
        }

        let unrestored = match rewound {
            Some(Ok(rewound)) => (!rewound.failed.is_empty()).then(|| {
                rewound
                    .failed
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<String>>()
                    .join("; ")
            }),
            Some(Err(reason)) => Some(reason),
            None => None,
        };
        if let Some(reason) = unrestored {
            let message = tr!("ade.chat.unrestored", reason = &reason);
            self.report_error(message.into(), Some(thread));
        } else if let Some(reason) = unsent.or(refused) {
            self.report_error(Arc::from(reason), Some(thread));
        }
    }

    fn sent_turn(&self, turn: u64) -> Option<Arc<Turn>> {
        self.thread()?
            .turns()
            .iter()
            .find(|sent| sent.stamp() == turn && sent.speaker() == Speaker::User)
            .cloned()
    }

    fn history(&self) -> Option<History> {
        let store = self.store.as_ref()?;
        Some(store.history(&self.open))
    }
}

fn changed_files(history: &History, since: u64, root: &Path) -> Vec<String> {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_owned());
    match history.changed_since(since) {
        Ok(changed) => changed
            .iter()
            .map(|path| {
                path.strip_prefix(&root)
                    .unwrap_or(path)
                    .display()
                    .to_string()
            })
            .collect(),
        Err(error) => {
            tracing::warn!(%error, "failed to read conversation file history");
            Vec::new()
        }
    }
}

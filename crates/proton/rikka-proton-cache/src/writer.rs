use rikka_sync::Mutex;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender};

use rusqlite::Connection;

use crate::error::{Error, Result};
use crate::overlay::{CacheKey, Overlay, Settled};
use crate::store;

const BATCH: usize = 512;
const BACKLOG: usize = 8192;

pub(crate) enum Op {
    Put {
        cache_key: CacheKey,
        key: Vec<u8>,
        sealed: Vec<u8>,
        used: u64,
        limit: Option<usize>,
    },
    TouchRecord {
        table: &'static str,
        key: Vec<u8>,
        used: u64,
    },
    Remove {
        cache_key: CacheKey,
        key: Vec<u8>,
    },
    Discard {
        table: &'static str,
        key: Vec<u8>,
        sealed: Vec<u8>,
    },
    ClearTable(&'static str),
    PutBlob {
        cache_key: CacheKey,
        key: Vec<u8>,
        sealed: Vec<u8>,
        used: u64,
        limit: u64,
    },
    TouchBlob {
        area: &'static str,
        key: Vec<u8>,
        used: u64,
    },
    RemoveBlob {
        cache_key: CacheKey,
        key: Vec<u8>,
    },
    Wipe,
}

pub(crate) enum Message {
    Ops { era: u64, ops: Vec<(Op, u64)> },
    Flush(Sender<Result<()>>),
}

pub(crate) struct State {
    pub(crate) overlay: Mutex<Overlay>,
    pub(crate) era: AtomicU64,
}

impl State {
    pub(crate) fn overlay(&self) -> rikka_sync::MutexGuard<'_, Overlay> {
        self.overlay.lock()
    }
}

pub(crate) fn spawn(
    connection: Connection,
    state: Arc<State>,
    messages: Receiver<Message>,
) -> Result<()> {
    std::thread::Builder::new()
        .name("rikka-proton-cache".to_owned())
        .spawn(move || {
            Writer {
                connection,
                state,
                backlog: Vec::new(),
            }
            .run(&messages);
        })
        .map(drop)
        .map_err(|error| Error::Database(error.to_string()))
}

struct Writer {
    connection: Connection,
    state: Arc<State>,
    backlog: Vec<Batch>,
}

struct Batch {
    era: u64,
    ops: Vec<(Op, u64)>,
}

impl Writer {
    fn run(mut self, messages: &Receiver<Message>) {
        let mut failure: Option<String> = None;
        while let Ok(first) = messages.recv() {
            let mut merged = vec![first];
            while merged.len() < BATCH {
                match messages.try_recv() {
                    Ok(message) => merged.push(message),
                    Err(_) => break,
                }
            }

            let mut flushes = Vec::new();
            for message in merged {
                match message {
                    Message::Ops { era, ops } => self.backlog.push(Batch { era, ops }),
                    Message::Flush(reply) => flushes.push(reply),
                }
            }

            if !self.backlog.is_empty() {
                failure = self.commit().err().map(|error| error.to_string());
            }

            for reply in flushes {
                let outcome = match &failure {
                    Some(failed) => Err(Error::Write(failed.clone())),
                    None => Ok(()),
                };
                drop(reply.send(outcome));
            }
        }
    }

    fn commit(&mut self) -> Result<()> {
        let era = self.state.era.load(Ordering::SeqCst);
        let written = (|| -> Result<()> {
            let transaction = self.connection.unchecked_transaction()?;
            let mut bounded: BTreeMap<&'static str, usize> = BTreeMap::new();
            for batch in &self.backlog {
                let current = batch.era == era;
                for (op, _) in &batch.ops {
                    apply(&transaction, op, current)?;
                    if let Op::Put {
                        cache_key,
                        limit: Some(limit),
                        ..
                    } = op
                        && current
                    {
                        bounded.insert(cache_key.table, *limit);
                    }
                }
            }
            for (table, limit) in bounded {
                store::evict_records(&transaction, table, limit)?;
            }
            transaction.commit()?;
            Ok(())
        })();

        match written {
            Ok(()) => {
                let committed = self.settled();
                self.state.overlay().commit_writes(&committed);
                Ok(())
            }
            Err(error) => {
                tracing::warn!(%error, "failed to write cache batch");
                if self
                    .backlog
                    .iter()
                    .map(|batch| batch.ops.len())
                    .sum::<usize>()
                    > BACKLOG
                {
                    let dropped = self.settled();
                    self.state.overlay().discard_writes(&dropped);
                }
                Err(error)
            }
        }
    }

    fn settled(&mut self) -> Vec<Settled> {
        self.backlog
            .drain(..)
            .flat_map(|batch| batch.ops)
            .filter_map(|(op, seq)| op.settled(seq))
            .collect()
    }
}

impl Op {
    fn settled(self, seq: u64) -> Option<Settled> {
        match self {
            Op::Put { cache_key, .. } | Op::Remove { cache_key, .. } => {
                Some(Settled::Record(cache_key, seq))
            }
            Op::PutBlob { cache_key, .. } | Op::RemoveBlob { cache_key, .. } => {
                Some(Settled::Blob(cache_key, seq))
            }
            Op::ClearTable(table) => Some(Settled::Table(table, seq)),
            Op::Wipe => Some(Settled::Wipe(seq)),
            Op::Discard { .. } | Op::TouchRecord { .. } | Op::TouchBlob { .. } => None,
        }
    }
}

fn apply(connection: &Connection, op: &Op, current: bool) -> Result<()> {
    match op {
        Op::Wipe => store::wipe(connection),
        _ if !current => Ok(()),
        Op::Put {
            cache_key,
            key,
            sealed,
            used,
            ..
        } => store::put(connection, cache_key.table, key, sealed, *used),
        Op::TouchRecord { table, key, used } => store::touch_record(connection, table, key, *used),
        Op::Remove { cache_key, key } => store::remove(connection, cache_key.table, key),
        Op::Discard { table, key, sealed } => store::discard(connection, table, key, sealed),
        Op::ClearTable(table) => store::clear_table(connection, table),
        Op::PutBlob {
            cache_key,
            key,
            sealed,
            used,
            limit,
        } => {
            store::put_blob(connection, cache_key.table, key, sealed, *used)?;
            store::evict(connection, cache_key.table, *limit)
        }
        Op::TouchBlob { area, key, used } => store::touch_blob(connection, area, key, *used),
        Op::RemoveBlob { cache_key, key } => store::remove_blob(connection, cache_key.table, key),
    }
}

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};

use bytemuck::Zeroable;

use crate::draw::instance::GpuInstance;

const KEEP_CHANGES: u64 = 64;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Change {
    pub version: u64,
    pub first: u32,
    pub count: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Span {
    pub first: u32,
    pub count: u32,
    pub room: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Move {
    pub from: u32,
    pub to: u32,
    pub count: u32,
}

#[derive(Debug)]
pub(crate) struct Relocation {
    pub from: u64,
    pub version: u64,
    pub moves: Vec<Move>,
}

#[derive(Debug)]
pub(crate) struct Ledger {
    pub epoch: u64,
    pub version: u64,
    changes: VecDeque<Change>,
    pub relocation: Option<Relocation>,
    dropped: u64,
    noted: u64,
}

impl Default for Ledger {
    fn default() -> Self {
        Self {
            epoch: fresh_epoch(),
            version: 0,
            changes: VecDeque::new(),
            relocation: None,
            dropped: 0,
            noted: 0,
        }
    }
}

fn fresh_epoch() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

impl Ledger {
    pub(crate) fn open(&mut self, size: usize) {
        self.version += 1;
        let earliest = self.version.saturating_sub(KEEP_CHANGES);
        while let Some(change) = self
            .changes
            .pop_front_if(|change| change.version <= earliest || self.noted > size as u64)
        {
            self.noted -= u64::from(change.count);
            self.dropped = change.version;
        }
    }

    pub(crate) fn since(&self) -> u64 {
        self.dropped
    }

    pub(crate) fn changes_after(&self, version: u64) -> impl Iterator<Item = &Change> {
        let first = self
            .changes
            .partition_point(|change| change.version <= version);
        self.changes.range(first..)
    }

    pub(crate) fn mark_relaid(&mut self) {
        self.epoch = fresh_epoch();
        self.changes.clear();
        self.noted = 0;
        self.dropped = self.version;
        self.relocation = None;
    }

    fn relocate(&mut self, mut moves: Vec<Move>) {
        moves.retain(|moved| moved.count > 0);
        let mut by_source = moves.clone();
        by_source.sort_unstable_by_key(|moved| moved.from);
        let version = self.version;
        let changes: VecDeque<Change> = self
            .changes_after(version.saturating_sub(1))
            .filter_map(|change| {
                let at = by_source.partition_point(|moved| moved.from <= change.first);
                let moved = by_source.get(at.checked_sub(1)?)?;
                let offset = change.first - moved.from;
                (offset < moved.count).then(|| Change {
                    version,
                    first: moved.to + offset,
                    count: change.count.min(moved.count - offset),
                })
            })
            .collect();
        self.relocation = Some(Relocation {
            from: self.epoch,
            version,
            moves,
        });
        self.epoch = fresh_epoch();
        self.noted = changes.iter().map(|change| u64::from(change.count)).sum();
        self.changes = changes;
        self.dropped = version.saturating_sub(1);
    }

    pub(crate) fn note(&mut self, first: u32, count: u32) {
        if count == 0 {
            return;
        }
        self.noted += u64::from(count);
        self.changes.push_back(Change {
            version: self.version,
            first,
            count,
        });
    }
}

#[derive(Debug, Default)]
pub(crate) struct Arena {
    pub data: Vec<GpuInstance>,
    pub used: u32,
    pub ledger: Ledger,
}

pub(crate) struct ArenaWrite {
    pub span: Span,
    pub moved: bool,
}

impl Arena {
    pub(crate) fn relayout(&mut self) {
        self.data.clear();
        self.used = 0;
        self.ledger.mark_relaid();
    }

    pub(crate) fn is_fragmented(&self) -> bool {
        self.data.len() > 1024 && u64::from(self.used) * 2 < self.data.len() as u64
    }

    pub(crate) fn write(&mut self, current: Span, instances: &[GpuInstance]) -> ArenaWrite {
        let count = instances.len() as u32;
        if count <= current.room {
            let at = current.first as usize;
            let reach = count.max(current.count) as usize;
            let (drawn, left) = self.data[at..at + reach].split_at_mut(count as usize);
            let same = left.is_empty()
                && bytemuck::cast_slice::<_, u8>(drawn) == bytemuck::cast_slice::<_, u8>(instances);
            if !same {
                drawn.copy_from_slice(instances);
                left.fill(GpuInstance::zeroed());
                self.ledger.note(current.first, reach as u32);
            }
            return ArenaWrite {
                span: Span { count, ..current },
                moved: false,
            };
        }
        self.release(current);
        let room = match current.room {
            0 => count,
            _ => count + count / 2,
        };
        let first = self.data.len() as u32;
        self.data.extend_from_slice(instances);
        self.data.resize(
            self.data.len() + (room - count) as usize,
            GpuInstance::zeroed(),
        );
        self.used += room;
        self.ledger.note(first, room);
        ArenaWrite {
            span: Span { first, count, room },
            moved: true,
        }
    }

    pub(crate) fn clear(&mut self, current: Span) -> Span {
        let at = current.first as usize;
        let drawn = &mut self.data[at..at + current.count as usize];
        drawn.fill(GpuInstance::zeroed());
        self.ledger.note(current.first, current.count);
        Span {
            count: 0,
            ..current
        }
    }

    pub(crate) fn release(&mut self, current: Span) {
        self.used -= current.room;
    }

    pub(crate) fn compact<'a>(&mut self, spans: impl Iterator<Item = &'a mut Span>) {
        let mut data = Vec::with_capacity(self.used as usize);
        let mut moves: Vec<Move> = Vec::new();
        for span in spans {
            let at = span.first as usize;
            let first = data.len() as u32;
            data.extend_from_slice(&self.data[at..at + span.room as usize]);
            match moves.last_mut() {
                Some(last)
                    if last.from + last.count == span.first && last.to + last.count == first =>
                {
                    last.count += span.room;
                }
                _ => moves.push(Move {
                    from: span.first,
                    to: first,
                    count: span.room,
                }),
            }
            span.first = first;
        }
        self.used = data.len() as u32;
        self.data = data;
        self.ledger.relocate(moves);
    }

    pub(crate) fn append(&mut self, instances: &[GpuInstance]) -> Span {
        let first = self.data.len() as u32;
        let count = instances.len() as u32;
        self.data.extend_from_slice(instances);
        self.used = self.data.len() as u32;
        Span {
            first,
            count,
            room: count,
        }
    }
}

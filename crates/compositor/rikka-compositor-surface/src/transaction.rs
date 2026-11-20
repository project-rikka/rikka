use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};
use std::time::Duration;

use slotmap::{SecondaryMap, SlotMap, new_key_type};
use smallvec::SmallVec;

use crate::blocker::Blockers;
use crate::buffer::Buffers;
use crate::ids::{BlockerId, SurfaceId};
use crate::state::Update;

new_key_type! {
    pub(crate) struct TransactionId;
}

pub(crate) type Entries = SmallVec<[(SurfaceId, Box<Update>); 1]>;

#[derive(Debug)]
pub(crate) struct Transaction {
    pub(crate) entries: Entries,
    pub(crate) blockers: SmallVec<[BlockerId; 2]>,
    remaining: u32,
    order: u64,
    marked: bool,
    constrained: Option<u64>,
}

#[derive(Debug, Default)]
pub(crate) struct Queue {
    transactions: SlotMap<TransactionId, Transaction>,
    lines: SecondaryMap<SurfaceId, VecDeque<TransactionId>>,
    blocked: SecondaryMap<BlockerId, TransactionId>,
    candidates: BinaryHeap<Reverse<(u64, TransactionId)>>,
    constrained: Vec<(TransactionId, u64)>,
    deadlines: BinaryHeap<Reverse<(Duration, u64, TransactionId)>>,
    orders: u64,
    epochs: u64,
    draining: bool,
}

impl Queue {
    pub(crate) fn len(&self) -> usize {
        self.transactions.len()
    }

    pub(crate) fn waiting_on(&self, surface: SurfaceId) -> usize {
        self.lines.get(surface).map_or(0, VecDeque::len)
    }

    pub(crate) fn push(&mut self, mut entries: Entries, active: impl Fn(BlockerId) -> bool) {
        let mut blockers = SmallVec::new();
        for (_, update) in &mut entries {
            blockers.extend(update.fences.drain(..).filter(|blocker| active(*blocker)));
            blockers.extend(update.blockers.drain(..).filter(|blocker| active(*blocker)));
        }
        self.orders += 1;
        let remaining = u32::try_from(blockers.len()).unwrap_or(u32::MAX);
        let id = self.transactions.insert(Transaction {
            entries,
            blockers,
            remaining,
            order: self.orders,
            marked: false,
            constrained: None,
        });
        let transaction = &self.transactions[id];
        for blocker in &transaction.blockers {
            self.blocked.insert(*blocker, id);
        }
        for (surface, _) in &transaction.entries {
            match self.lines.entry(*surface) {
                Some(line) => line.or_default().push_back(id),
                None => continue,
            }
        }
        self.mark(id);
    }

    pub(crate) fn unblocked(&mut self, blocker: BlockerId) {
        let Some(id) = self.blocked.remove(blocker) else {
            return;
        };
        let Some(transaction) = self.transactions.get_mut(id) else {
            return;
        };
        transaction.remaining = transaction.remaining.saturating_sub(1);
        if transaction.remaining == 0 {
            self.mark(id);
        }
    }

    pub(crate) fn next_candidate(&mut self) -> Option<TransactionId> {
        while let Some(Reverse((_, id))) = self.candidates.pop() {
            let Some(transaction) = self.transactions.get_mut(id) else {
                continue;
            };
            transaction.marked = false;
            if transaction.remaining > 0 || !at_heads(&self.lines, id, transaction) {
                continue;
            }
            return Some(id);
        }
        None
    }

    pub(crate) fn get(&self, id: TransactionId) -> Option<&Transaction> {
        self.transactions.get(id)
    }

    pub(crate) fn constrain(&mut self, id: TransactionId, deadline: Option<Duration>) {
        let Some(transaction) = self.transactions.get_mut(id) else {
            return;
        };
        self.epochs += 1;
        transaction.constrained = Some(self.epochs);
        self.constrained.push((id, self.epochs));
        if let Some(at) = deadline {
            self.deadlines.push(Reverse((at, self.epochs, id)));
        }
    }

    pub(crate) fn reconsider(&mut self) {
        for (id, epoch) in std::mem::take(&mut self.constrained) {
            if self.constrained_since(id) == Some(epoch) {
                self.mark(id);
            }
        }
    }

    pub(crate) fn prune_constrained(&mut self) {
        let transactions = &self.transactions;
        self.constrained.retain(|(id, epoch)| {
            transactions
                .get(*id)
                .is_some_and(|transaction| transaction.constrained == Some(*epoch))
        });
    }

    pub(crate) fn constrained(&self) -> impl Iterator<Item = &Transaction> {
        self.constrained
            .iter()
            .filter_map(|(id, _)| self.transactions.get(*id))
    }

    fn constrained_since(&self, id: TransactionId) -> Option<u64> {
        self.transactions.get(id)?.constrained
    }

    pub(crate) fn next_deadline(&mut self) -> Option<Duration> {
        while let Some(Reverse((at, epoch, id))) = self.deadlines.peek().copied() {
            if self.constrained_since(id) == Some(epoch) {
                return Some(at);
            }
            self.deadlines.pop();
        }
        None
    }

    pub(crate) fn take(&mut self, id: TransactionId) -> Option<Transaction> {
        let transaction = self.transactions.remove(id)?;
        for (surface, _) in &transaction.entries {
            let Some(line) = self.lines.get_mut(*surface) else {
                continue;
            };
            if line.front() == Some(&id) {
                line.pop_front();
            }
            if let Some(&next) = line.front() {
                self.mark(next);
            }
        }
        for blocker in &transaction.blockers {
            if self.blocked.get(*blocker) == Some(&id) {
                self.blocked.remove(*blocker);
            }
        }
        Some(transaction)
    }

    pub(crate) fn forget(
        &mut self,
        surface: SurfaceId,
        buffers: &mut Buffers,
        blockers: &mut Blockers,
    ) {
        let Some(line) = self.lines.remove(surface) else {
            return;
        };
        for id in line {
            let Some(transaction) = self.transactions.get_mut(id) else {
                continue;
            };
            if let Some(at) = transaction
                .entries
                .iter()
                .position(|(entry, _)| *entry == surface)
            {
                let (_, mut update) = transaction.entries.remove(at);
                update.discard(buffers, blockers);
            }
            if !transaction.entries.is_empty() {
                self.mark(id);
                continue;
            }
            if let Some(transaction) = self.take(id) {
                blockers.release_all(transaction.blockers);
            }
        }
    }

    pub(crate) fn begin_drain(&mut self) -> bool {
        !std::mem::replace(&mut self.draining, true)
    }

    pub(crate) fn end_drain(&mut self) {
        self.draining = false;
    }

    fn mark(&mut self, id: TransactionId) {
        let Some(transaction) = self.transactions.get_mut(id) else {
            return;
        };
        transaction.constrained = None;
        if std::mem::replace(&mut transaction.marked, true) {
            return;
        }
        self.candidates.push(Reverse((transaction.order, id)));
    }
}

fn at_heads(
    lines: &SecondaryMap<SurfaceId, VecDeque<TransactionId>>,
    id: TransactionId,
    transaction: &Transaction,
) -> bool {
    transaction.entries.iter().all(|(surface, _)| {
        lines
            .get(*surface)
            .is_none_or(|line| line.front() == Some(&id))
    })
}

use std::cell::RefCell;
use std::io::{Read, Write};
use std::sync::Arc;

use crate::address::Address;
use crate::error::{Error, Result};
use crate::item::Item;
use crate::job;
use crate::operation::{
    Conflict, Failure, Inner, Operation, Outcome, Phase, Placement, Resolution, Shared, Task,
};
use crate::plan::Plan;
use crate::store::Store;

const CHUNK: usize = 128 * 1024;

const BATCH: usize = 1024;

#[must_use]
pub fn run<S: Store>(store: Arc<S>, task: Task<S::Location>) -> Operation<S::Item> {
    job::spawn(task.kind(), move |reporter| {
        Runner {
            store,
            shared: Arc::clone(reporter.shared()),
            buffer: RefCell::new(vec![0; CHUNK]),
        }
        .run(&task)
    })
}

struct Runner<S: Store> {
    store: Arc<S>,
    shared: Arc<Shared<S::Item>>,
    buffer: RefCell<Vec<u8>>,
}

type Located<S> = <S as Store>::Location;

impl<S: Store> Runner<S> {
    fn run(&self, task: &Task<S::Location>) -> Outcome<S::Location> {
        let mut outcome = Outcome::default();
        let work = Work::list(task);

        let plan = self.prepare(&work, &mut outcome);
        if self.stopped() {
            outcome.cancelled = true;
            return outcome;
        }
        self.update(|inner| {
            inner.progress.phase = Phase::Working;
            inner.progress.items_total = plan.items;
            inner.progress.bytes_total = plan.bytes;
        });

        let unmeasured: Vec<S::Location> = outcome
            .failures
            .iter()
            .map(|failure| failure.at.clone())
            .collect();
        for item in &work {
            if self.stopped() {
                outcome.cancelled = true;
                break;
            }
            if unmeasured.contains(item.source()) {
                continue;
            }
            let mut batch = Vec::new();
            let result = match *item {
                Work::Transfer {
                    source,
                    target,
                    moving,
                } => self.transfer(source, target, moving, &mut batch, &mut outcome),
                Work::Trash(at) => self.run_single(at, S::trash, &mut outcome),
                Work::Delete(at) => self.remove(at),
                Work::Restore(at) => self.run_single(at, S::restore, &mut outcome),
            }
            .and_then(|()| self.flush(&mut batch, &mut outcome));
            if let Err(Error::Cancelled) = result {
                outcome.cancelled = true;
                break;
            }
            if let Err(error) = result {
                outcome
                    .failures
                    .push(Failure::new(item.source().clone(), error));
            }
        }
        outcome
    }

    fn prepare(&self, work: &[Work<'_, S::Location>], outcome: &mut Outcome<S::Location>) -> Plan {
        let mut plan = Plan::default();
        for item in work {
            if self.stopped() {
                break;
            }
            match self.measure(item.source(), item.is_deep()) {
                Ok(measured) => plan.add(measured),
                Err(error) => outcome
                    .failures
                    .push(Failure::new(item.source().clone(), error)),
            }
        }
        plan
    }

    fn measure(&self, at: &S::Location, deep: bool) -> Result<Plan, Located<S>> {
        let entry = self.store.stat(at)?;
        self.measure_entry(&entry, deep)
    }

    fn measure_entry(&self, entry: &S::Item, deep: bool) -> Result<Plan, Located<S>> {
        let mut plan = Plan {
            items: 1,
            bytes: entry.size(),
        };
        if deep && entry.is_directory() && entry.link().is_none() {
            for child in self.store.walk(entry.location())? {
                if self.stopped() {
                    break;
                }
                if let Ok(measured) = self.measure_entry(&child, true) {
                    plan.add(measured);
                }
            }
        }
        Ok(plan)
    }

    fn transfer(
        &self,
        source: &S::Location,
        target: Target<'_, S::Location>,
        moving: bool,
        batch: &mut Vec<Pending<S::Item>>,
        outcome: &mut Outcome<S::Location>,
    ) -> Result<(), Located<S>> {
        let (Target::Into(bound) | Target::At(bound)) = target;
        if source.contains(bound) {
            return Err(Error::IntoItself { at: source.clone() });
        }
        let entry = self.store.stat(source)?;
        let destination = match target {
            Target::Into(into) => into.child(entry.name()),
            Target::At(at) => {
                self.ensure_parent(at)?;
                at.clone()
            }
        };
        let placing = Placing {
            moving,
            recorded: true,
        };
        self.place(&entry, &destination, placing, batch, outcome)
    }

    fn ensure_parent(&self, at: &S::Location) -> Result<(), Located<S>> {
        let Some(parent) = at.parent() else {
            return Ok(());
        };
        match self.store.stat(&parent) {
            Err(Error::NotFound { .. }) => {
                self.ensure_parent(&parent)?;
                self.store.create_directory(&parent)
            }
            found => found.map(drop),
        }
    }

    fn place(
        &self,
        entry: &S::Item,
        destination: &S::Location,
        placing: Placing,
        batch: &mut Vec<Pending<S::Item>>,
        outcome: &mut Outcome<S::Location>,
    ) -> Result<(), Located<S>> {
        let Some(Destination {
            at: destination,
            merging,
        }) = self.resolve_destination(entry, destination, placing)?
        else {
            self.advance(entry);
            return Ok(());
        };
        let source = entry.location();
        if placing.moving && !merging && self.store.can_rename_across(source, &destination) {
            self.store.rename_across(source, &destination)?;
            if placing.recorded {
                outcome
                    .placed
                    .push(Placement::new(source.clone(), destination));
            }
            self.advance(entry);
            return Ok(());
        }

        if let Some(target) = entry.link_target() {
            self.store.link(target, &destination)?;
            if placing.recorded {
                outcome
                    .placed
                    .push(Placement::new(source.clone(), destination));
            }
            self.advance(entry);
            if placing.moving {
                self.store.delete(source)?;
            }
            return Ok(());
        }
        if entry.is_directory() {
            return self.copy_tree(entry, &destination, merging, placing, outcome);
        }
        batch.push(Pending {
            entry: entry.clone(),
            destination,
            placing,
        });
        if batch.len() >= BATCH {
            self.flush(batch, outcome)?;
        }
        Ok(())
    }

    fn copy_tree(
        &self,
        entry: &S::Item,
        destination: &S::Location,
        merging: bool,
        placing: Placing,
        outcome: &mut Outcome<S::Location>,
    ) -> Result<(), Located<S>> {
        if !merging {
            self.store.create_directory(destination)?;
            if placing.recorded {
                outcome.placed.push(Placement::new(
                    entry.location().clone(),
                    destination.clone(),
                ));
            }
        }
        self.advance(entry);

        let children = self.store.walk(entry.location())?;
        let inside = Placing {
            moving: placing.moving,
            recorded: placing.recorded && merging,
        };
        let mut batch = Vec::new();
        for child in children {
            if self.stopped() {
                return Err(Error::Cancelled);
            }
            let into = destination.child(child.name());
            if let Err(error) = self.place(&child, &into, inside, &mut batch, outcome) {
                if matches!(error, Error::Cancelled) {
                    return Err(error);
                }
                outcome
                    .failures
                    .push(Failure::new(child.location().clone(), error));
            }
        }
        self.flush(&mut batch, outcome)?;

        self.copy_metadata(entry, destination)?;
        if placing.moving {
            match self.store.delete(entry.location()) {
                Err(Error::NotEmpty { .. }) => {}
                deleted => deleted?,
            }
        }
        Ok(())
    }

    fn flush(
        &self,
        batch: &mut Vec<Pending<S::Item>>,
        outcome: &mut Outcome<S::Location>,
    ) -> Result<(), Located<S>> {
        let pending = std::mem::take(batch);
        if pending.is_empty() {
            return Ok(());
        }
        let sources: Vec<S::Location> = pending
            .iter()
            .map(|pending| pending.entry.location().clone())
            .collect();
        let mut failures = Vec::new();
        let mut placed = Vec::new();
        let mut served = vec![false; pending.len()];
        let read = self.store.read_each(&sources, &mut |position, source| {
            let Some(pending) = pending.get(position) else {
                return Ok(());
            };
            served[position] = true;
            match source.and_then(|source| self.copy_file(pending, source, &mut placed)) {
                Err(Error::Cancelled) => Err(Error::Cancelled),
                Err(error) => {
                    failures.push(Failure::new(pending.entry.location().clone(), error));
                    Ok(())
                }
                Ok(()) => Ok(()),
            }
        });
        let stopped = match read {
            Err(Error::Cancelled) => Some(Error::Cancelled),
            Err(error) => {
                failures.extend(
                    pending
                        .iter()
                        .zip(&served)
                        .filter(|(_, served)| !**served)
                        .map(|(pending, _)| {
                            Failure::new(pending.entry.location().clone(), error.clone())
                        }),
                );
                None
            }
            Ok(()) => None,
        };
        outcome.failures.extend(failures);
        outcome.placed.extend(placed);
        match stopped {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn copy_file(
        &self,
        pending: &Pending<S::Item>,
        source: &mut dyn Read,
        placed: &mut Vec<Placement<S::Location>>,
    ) -> Result<(), Located<S>> {
        let Pending {
            entry,
            destination,
            placing,
        } = pending;
        let mut sink = self.store.write(destination)?;
        let mut buffer = self.buffer.borrow_mut();
        loop {
            if self.stopped() {
                return Err(Error::Cancelled);
            }
            let read = match source.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => read,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(Error::from_io(&error, entry.location())),
            };
            sink.write_all(&buffer[..read])
                .map_err(|error| Error::from_io(&error, destination))?;
            self.update(|inner| inner.progress.bytes_done += read as u64);
        }
        sink.commit()
            .map_err(|error| Error::from_io(&error, destination))?;
        if placing.recorded {
            placed.push(Placement::new(
                entry.location().clone(),
                destination.clone(),
            ));
        }

        self.copy_metadata(entry, destination)?;
        self.update(|inner| inner.progress.items_done += 1);
        if placing.moving {
            self.store.delete(entry.location())?;
        }
        Ok(())
    }

    fn remove(&self, at: &S::Location) -> Result<(), Located<S>> {
        let entry = self.store.stat(at)?;
        self.remove_entry(&entry)
    }

    fn remove_entry(&self, entry: &S::Item) -> Result<(), Located<S>> {
        if entry.is_directory() && entry.link().is_none() {
            for child in self.store.walk(entry.location())? {
                if self.stopped() {
                    return Err(Error::Cancelled);
                }
                self.remove_entry(&child)?;
            }
        }
        self.store.delete(entry.location())?;
        self.advance(entry);
        Ok(())
    }

    fn run_single(
        &self,
        at: &S::Location,
        action: impl Fn(&S, &S::Location) -> Result<S::Location, Located<S>>,
        outcome: &mut Outcome<S::Location>,
    ) -> Result<(), Located<S>> {
        let entry = self.store.stat(at)?;
        let landed = action(&self.store, at)?;
        outcome.placed.push(Placement::new(at.clone(), landed));
        self.advance(&entry);
        Ok(())
    }

    fn resolve_destination(
        &self,
        entry: &S::Item,
        destination: &S::Location,
        placing: Placing,
    ) -> Result<Option<Destination<S::Location>>, Located<S>> {
        let Ok(existing) = self.store.stat(destination) else {
            return Ok(Some(Destination::fresh(destination.clone())));
        };
        if existing.location() == entry.location() {
            return Ok((!placing.moving).then(|| self.beside(entry, destination)));
        }
        if entry.is_directory() && entry.link().is_none() && existing.is_directory() {
            return Ok(Some(Destination {
                at: destination.clone(),
                merging: true,
            }));
        }
        let conflict = Conflict {
            source: entry.clone(),
            existing,
            destination: destination.clone(),
        };
        match self.ask(conflict) {
            Resolution::Replace => Ok(Some(Destination::fresh(destination.clone()))),
            Resolution::Skip => Ok(None),
            Resolution::Cancel => Err(Error::Cancelled),
            Resolution::Keep => Ok(Some(self.beside(entry, destination))),
        }
    }

    fn beside(&self, entry: &S::Item, destination: &S::Location) -> Destination<S::Location> {
        let parent = destination.parent().unwrap_or_else(|| destination.clone());
        let free = rikka_format::unique_name(entry.name(), |candidate| {
            self.store.stat(&parent.child(candidate)).is_ok()
        });
        Destination::fresh(parent.child(&free))
    }

    fn ask(&self, conflict: Conflict<S::Item>) -> Resolution {
        if let Some(standing) = self.shared.lock().standing {
            return standing;
        }
        self.shared.update(|inner| {
            inner.progress.phase = Phase::Asking;
            inner.question = Some(conflict);
            inner.answer = None;
        });
        let answer = self.shared.wait_for_answer();
        self.shared.update(|inner| {
            inner.question = None;
            inner.progress.phase = Phase::Working;
        });
        answer
    }

    fn copy_metadata(&self, entry: &S::Item, destination: &S::Location) -> Result<(), Located<S>> {
        match self.store.copy_metadata(entry, destination) {
            Err(Error::Denied { .. } | Error::Unsupported { .. }) => Ok(()),
            copied => copied,
        }
    }

    fn advance(&self, entry: &S::Item) {
        let size = entry.size();
        self.update(|inner| {
            inner.progress.items_done += 1;
            inner.progress.bytes_done += size;
        });
    }

    fn update(&self, change: impl FnOnce(&mut Inner<S::Item>)) {
        self.shared.update(change);
    }

    fn stopped(&self) -> bool {
        self.shared.is_cancelled()
    }
}

enum Work<'a, L: Address> {
    Transfer {
        source: &'a L,
        target: Target<'a, L>,
        moving: bool,
    },
    Trash(&'a L),
    Delete(&'a L),
    Restore(&'a L),
}

impl<L: Address> Clone for Work<'_, L> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<L: Address> Copy for Work<'_, L> {}

impl<'a, L: Address> Work<'a, L> {
    fn list(task: &'a Task<L>) -> Vec<Self> {
        let transfers = |sources: &'a [L], into: &'a L, moving: bool| {
            sources
                .iter()
                .map(|source| Self::Transfer {
                    source,
                    target: Target::Into(into),
                    moving,
                })
                .collect()
        };
        match task {
            Task::Copy { sources, into } => transfers(sources, into, false),
            Task::Move { sources, into } => transfers(sources, into, true),
            Task::Relocate { placements } => placements
                .iter()
                .map(|placement| Self::Transfer {
                    source: &placement.from,
                    target: Target::At(&placement.to),
                    moving: true,
                })
                .collect(),
            Task::Trash { items } => items.iter().map(Self::Trash).collect(),
            Task::Delete { items } => items.iter().map(Self::Delete).collect(),
            Task::Restore { items } => items.iter().map(Self::Restore).collect(),
        }
    }

    fn source(&self) -> &'a L {
        match *self {
            Self::Transfer { source, .. } => source,
            Self::Trash(at) | Self::Delete(at) | Self::Restore(at) => at,
        }
    }

    fn is_deep(&self) -> bool {
        matches!(self, Self::Transfer { .. } | Self::Delete(_))
    }
}

enum Target<'a, L> {
    Into(&'a L),
    At(&'a L),
}

impl<L> Clone for Target<'_, L> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<L> Copy for Target<'_, L> {}

#[derive(Clone, Copy)]
struct Placing {
    moving: bool,
    recorded: bool,
}

struct Destination<L> {
    at: L,
    merging: bool,
}

impl<L> Destination<L> {
    fn fresh(at: L) -> Self {
        Self { at, merging: false }
    }
}

struct Pending<I: Item> {
    entry: I,
    destination: I::Location,
    placing: Placing,
}

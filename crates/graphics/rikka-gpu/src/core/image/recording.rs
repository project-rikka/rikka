use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::core::image::access::ImageAccess;

#[derive(Default)]
struct Ticket {
    generation: AtomicU64,
    committed: AtomicBool,
    abandoned: AtomicBool,
}

#[derive(Clone)]
struct Stamp {
    ticket: Arc<Ticket>,
    generation: u64,
}

impl Stamp {
    fn abandoned(&self) -> bool {
        self.ticket.generation.load(Ordering::Acquire) == self.generation
            && self.ticket.abandoned.load(Ordering::Acquire)
    }

    fn is(&self, other: &Stamp) -> bool {
        Arc::ptr_eq(&self.ticket, &other.ticket) && self.generation == other.generation
    }
}

thread_local! {
    static RECORDING: RefCell<Option<Stamp>> = const { RefCell::new(None) };
}

fn recording() -> Option<Stamp> {
    RECORDING.with(|current| current.borrow().clone())
}

pub(crate) struct Recording {
    stamp: Stamp,
    committed: bool,
}

#[derive(Default)]
pub(crate) struct Tickets {
    spare: Vec<Arc<Ticket>>,
}

const KEPT_TICKETS: usize = 4;

impl Tickets {
    fn take(&mut self) -> Stamp {
        let reusable = self.spare.iter().find(|ticket| {
            Arc::strong_count(ticket) == 1 || ticket.committed.load(Ordering::Acquire)
        });
        if let Some(free) = reusable {
            let generation = free.generation.fetch_add(1, Ordering::AcqRel) + 1;
            free.committed.store(false, Ordering::Release);
            free.abandoned.store(false, Ordering::Release);
            return Stamp {
                ticket: Arc::clone(free),
                generation,
            };
        }
        let ticket = Arc::new(Ticket::default());
        if self.spare.len() < KEPT_TICKETS {
            self.spare.push(Arc::clone(&ticket));
        }
        Stamp {
            ticket,
            generation: 0,
        }
    }
}

impl Recording {
    pub(crate) fn open(tickets: &mut Tickets) -> Self {
        let stamp = tickets.take();
        RECORDING.with(|current| *current.borrow_mut() = Some(stamp.clone()));
        Self {
            stamp,
            committed: false,
        }
    }

    pub(crate) fn commit(mut self) {
        self.committed = true;
    }
}

impl Drop for Recording {
    fn drop(&mut self) {
        let outcome = match self.committed {
            true => &self.stamp.ticket.committed,
            false => &self.stamp.ticket.abandoned,
        };
        outcome.store(true, Ordering::Release);
        RECORDING.with(|current| {
            let mut current = current.borrow_mut();
            if current.as_ref().is_some_and(|open| open.is(&self.stamp)) {
                *current = None;
            }
        });
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct LayoutState {
    pub(super) state: ImageAccess,
    pub(super) keeps: bool,
}

impl LayoutState {
    const FRESH: Self = Self {
        state: ImageAccess::UNDEFINED,
        keeps: false,
    };
}

pub(super) struct Tracking {
    settled: LayoutState,
    recorded: Option<(Stamp, LayoutState)>,
}

impl Tracking {
    pub(super) const fn fresh() -> Self {
        Self {
            settled: LayoutState::FRESH,
            recorded: None,
        }
    }

    pub(super) fn current(&self) -> LayoutState {
        match &self.recorded {
            Some((stamp, tracked)) if !stamp.abandoned() => *tracked,
            _ => self.settled,
        }
    }

    pub(super) fn set(&mut self, next: LayoutState) {
        let open = recording();
        if let Some((stamp, tracked)) = self.recorded.take() {
            if open.as_ref().is_some_and(|open| open.is(&stamp)) {
                self.recorded = Some((stamp, next));
                return;
            }
            if !stamp.abandoned() {
                self.settled = tracked;
            }
        }
        match open {
            Some(stamp) => self.recorded = Some((stamp, next)),
            None => self.settled = next,
        }
    }
}

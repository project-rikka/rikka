use rikka_sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Default)]
pub(crate) struct WriteGate {
    generation: AtomicU64,
    tickets: AtomicU64,
    writing: Mutex<()>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct WriteTicket {
    generation: u64,
    id: u64,
}

impl WriteGate {
    pub(crate) fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    pub(crate) fn ticket(&self, generation: u64) -> WriteTicket {
        WriteTicket {
            generation,
            id: self.tickets.fetch_add(1, Ordering::SeqCst) + 1,
        }
    }

    pub(crate) fn invalidate(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
    }

    pub(crate) fn write_if_current<T>(
        &self,
        ticket: WriteTicket,
        write: impl FnOnce() -> T,
    ) -> Option<T> {
        let _guard = self.writing.lock();
        let current = self.generation.load(Ordering::SeqCst) == ticket.generation
            && self.tickets.load(Ordering::SeqCst) == ticket.id;
        current.then(write)
    }

    pub(crate) fn exclusive<T>(&self, write: impl FnOnce() -> T) -> T {
        let _guard = self.writing.lock();
        write()
    }
}

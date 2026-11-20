use rikka_sync::Mutex;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use futures_channel::oneshot;

use crate::device::DeviceId;
use crate::event::Event;
use crate::prompt::{Prompt, Question, Reply};
use rikka_bus::EventSink;

pub(crate) struct Prompts {
    open: Mutex<HashMap<u64, Option<oneshot::Sender<Reply>>>>,
    next: AtomicU64,
    report: EventSink<Event>,
}

impl Prompts {
    pub(crate) fn new(report: EventSink<Event>) -> Prompts {
        Prompts {
            open: Mutex::new(HashMap::new()),
            next: AtomicU64::new(1),
            report,
        }
    }

    pub(crate) fn ask(&self, device: DeviceId, question: Question) -> oneshot::Receiver<Reply> {
        let (answer, reply) = oneshot::channel();
        self.open(device, question, Some(answer));
        reply
    }

    pub(crate) fn notify(&self, device: DeviceId, question: Question) {
        self.open(device, question, None);
    }

    fn open(&self, device: DeviceId, question: Question, answer: Option<oneshot::Sender<Reply>>) {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        self.lock().insert(id, answer);
        self.report.emit(Event::Asked(Prompt {
            id,
            device,
            question,
        }));
    }

    pub(crate) fn answer(&self, id: u64, reply: Reply) {
        let Some(answer) = self.lock().remove(&id) else {
            return;
        };
        if let Some(answer) = answer {
            let _ = answer.send(reply);
        }
        self.report.emit(Event::Withdrew(id));
    }

    pub(crate) fn withdraw(&self) {
        let open: Vec<u64> = {
            let mut pending = self.lock();
            let ids = pending.keys().copied().collect();
            pending.clear();
            ids
        };
        for id in open {
            self.report.emit(Event::Withdrew(id));
        }
    }

    fn lock(&self) -> rikka_sync::MutexGuard<'_, HashMap<u64, Option<oneshot::Sender<Reply>>>> {
        self.open.lock()
    }
}

impl std::fmt::Debug for Prompts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Prompts").finish_non_exhaustive()
    }
}

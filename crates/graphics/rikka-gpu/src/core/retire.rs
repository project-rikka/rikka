use std::any::Any;
use std::sync::mpsc::{Sender, channel};
use std::thread::JoinHandle;

type RetiredResource = Box<dyn Any + Send>;

#[derive(Default)]
pub struct RetireBin {
    entries: Vec<(u64, RetiredResource)>,
    due: Vec<RetiredResource>,
    drop_thread: Option<DropThread>,
}

struct DropThread {
    sender: Sender<Vec<RetiredResource>>,
    thread: JoinHandle<()>,
}

impl DropThread {
    fn start() -> Option<Self> {
        let (sender, taken) = channel::<Vec<RetiredResource>>();
        let spawned = std::thread::Builder::new()
            .name("rikka-reaper".into())
            .spawn(move || {
                while let Ok(batch) = taken.recv() {
                    drop(batch);
                }
            });
        match spawned {
            Ok(thread) => Some(Self { sender, thread }),
            Err(error) => {
                tracing::warn!("failed to start drop thread, freeing resources inline: {error}");
                None
            }
        }
    }
}

impl RetireBin {
    pub fn defer(&mut self, value: u64, resource: RetiredResource) {
        self.entries.push((value, resource));
    }

    pub(crate) fn collect_within_frame(&mut self, completed: u64) {
        let mut at = 0;
        while at < self.entries.len() {
            match self.entries[at].0 <= completed {
                true => self.due.push(self.entries.swap_remove(at).1),
                false => at += 1,
            }
        }
    }

    pub(crate) fn release_due(&mut self) {
        if self.due.is_empty() {
            return;
        }
        let drop_thread = match &mut self.drop_thread {
            Some(drop_thread) => drop_thread,
            empty => match DropThread::start() {
                Some(drop_thread) => empty.insert(drop_thread),
                None => {
                    self.due.clear();
                    return;
                }
            },
        };
        if let Err(returned) = drop_thread.sender.send(std::mem::take(&mut self.due)) {
            drop(returned.0);
        }
    }

    pub(crate) fn collect(&mut self, completed: u64) {
        self.due.clear();
        self.entries.retain(|(value, _)| *value > completed);
    }
}

impl Drop for RetireBin {
    fn drop(&mut self) {
        self.due.clear();
        if let Some(DropThread { sender, thread }) = self.drop_thread.take() {
            drop(sender);
            let _ = thread.join();
        }
    }
}

impl std::fmt::Debug for RetireBin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetireBin")
            .field("pending", &self.entries.len())
            .field("due", &self.due.len())
            .finish()
    }
}

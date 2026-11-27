use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use rikka_types::Size;
use rikka_ui::Pulse;

use crate::claim::Claim;
use crate::command::{Command, Link};
use crate::frame::Frame;
use crate::input::Input;
use crate::look::Look;
use crate::shared::Shared;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ViewId(u64);

impl ViewId {
    fn next_id() -> ViewId {
        ViewId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Clone, Debug)]
pub struct View(Arc<Inner>);

#[derive(Debug)]
struct Inner {
    id: ViewId,
    link: Link,
    shared: Arc<Shared>,
}

impl View {
    pub(crate) fn adopt(link: &Link, claim: Claim) -> View {
        let id = ViewId::next_id();
        let tick_link = link.clone();
        let pulse = Pulse::new(move |tick| {
            let _ = tick_link.send(Command::Tick { view: id, tick });
        });
        let shared = Arc::new(Shared::new(pulse));
        let _ = link.send(Command::Adopt {
            view: id,
            claim,
            shared: Arc::clone(&shared),
        });
        View(Arc::new(Inner {
            id,
            link: link.clone(),
            shared,
        }))
    }

    #[must_use]
    pub fn id(&self) -> ViewId {
        self.0.id
    }

    pub fn reclaim(&self, claim: Claim) {
        self.send(Command::Reclaim {
            view: self.0.id,
            claim,
        });
    }

    #[must_use]
    pub fn frame(&self) -> Arc<Frame> {
        self.0.shared.frame()
    }

    #[must_use]
    pub fn look(&self) -> Arc<Look> {
        self.0.shared.look()
    }

    pub fn resize(&self, size: Size, scale: f64) {
        self.send(Command::Resize {
            view: self.0.id,
            size,
            scale,
        });
    }

    pub fn set_visible(&self, visible: bool) {
        self.send(Command::Visible {
            view: self.0.id,
            visible,
        });
    }

    pub fn set_fullscreen(&self, fullscreen: bool) {
        self.send(Command::Fullscreen {
            view: self.0.id,
            fullscreen,
        });
    }

    pub fn close(&self) {
        self.send(Command::Close(self.0.id));
    }

    pub fn deliver(&self, input: Input) {
        self.send(Command::Input {
            view: self.0.id,
            input,
            at: Instant::now(),
        });
    }

    #[must_use]
    pub fn pulse(&self) -> &Pulse {
        self.0.shared.pulse()
    }

    pub fn watch(&self, on_change: impl Fn() + Send + Sync + 'static) {
        self.0.shared.watch(Box::new(on_change));
    }

    fn send(&self, command: Command) {
        let _ = self.0.link.send(command);
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        let _ = self.link.send(Command::Forget(self.id));
    }
}

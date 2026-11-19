#[cfg(unix)]
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
#[cfg(unix)]
use std::task::Waker;

#[cfg(unix)]
use rikka_theme::Theme;
use rikka_ui::RuntimeMessage;
use rikka_ui_host::TaskSink;
use rikka_worker::EventNotifier;

#[cfg(unix)]
use crate::wayland::settings::SessionChange;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct WindowId(u64);

impl WindowId {
    pub(crate) fn after(self) -> WindowId {
        WindowId(self.0 + 1)
    }
}

impl From<u64> for WindowId {
    fn from(raw: u64) -> WindowId {
        WindowId(raw)
    }
}

impl From<WindowId> for u64 {
    fn from(id: WindowId) -> u64 {
        id.0
    }
}

#[cfg(unix)]
impl rikka_wayland_client::Key for WindowId {
    fn into_raw(self) -> u64 {
        self.0
    }

    fn from_raw(raw: u64) -> WindowId {
        WindowId(raw)
    }
}

pub(crate) enum LoopEvent {
    Opened(Box<rikka_instance::LaunchRequest>),
    Message(TaskMessage),
    #[cfg(unix)]
    Theme(Box<Theme>),
    #[cfg(unix)]
    Language,
    #[cfg(unix)]
    Drag(Box<crate::wayland::data_device::DragEvent>),
    #[cfg(unix)]
    Presented(rikka_wayland::PresentationFeedback),
}

#[cfg(unix)]
impl From<SessionChange> for LoopEvent {
    fn from(change: SessionChange) -> Self {
        match change {
            SessionChange::Theme(theme) => LoopEvent::Theme(theme),
            SessionChange::Language => LoopEvent::Language,
        }
    }
}

pub(crate) struct TaskMessage {
    pub(crate) window: WindowId,
    pub(crate) inbound: RuntimeMessage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LoopClosed;

#[derive(Clone)]
pub(crate) struct LoopProxy {
    sender: Sender<LoopEvent>,
    notifier: EventNotifier,
}

pub(crate) struct LoopReceiver {
    receiver: Receiver<LoopEvent>,
}

pub(crate) fn mailbox(notifier: EventNotifier) -> (LoopProxy, LoopReceiver) {
    let (sender, receiver) = mpsc::channel();
    (LoopProxy { sender, notifier }, LoopReceiver { receiver })
}

impl LoopProxy {
    pub(crate) fn send_event(&self, event: LoopEvent) -> Result<(), LoopClosed> {
        self.sender.send(event).map_err(|_| LoopClosed)?;
        self.wake();
        Ok(())
    }

    #[cfg(unix)]
    pub(crate) fn waker(&self) -> Waker {
        Waker::from(Arc::new(self.notifier.clone()))
    }

    pub(crate) fn wake(&self) {
        if let Err(error) = self.notifier.notify() {
            tracing::debug!("failed to wake event loop: {error}");
        }
    }
}

impl LoopReceiver {
    pub(crate) fn take(&self) -> Option<LoopEvent> {
        self.receiver.try_recv().ok()
    }
}

pub(crate) struct LoopSink(LoopProxy);

impl LoopSink {
    pub(crate) fn new(proxy: LoopProxy) -> Self {
        Self(proxy)
    }
}

impl TaskSink<WindowId> for LoopSink {
    fn deliver(&self, window: WindowId, inbound: RuntimeMessage) -> bool {
        self.0
            .send_event(LoopEvent::Message(TaskMessage { window, inbound }))
            .is_ok()
    }
}

impl std::fmt::Debug for LoopSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoopSink").finish_non_exhaustive()
    }
}

impl std::fmt::Debug for LoopProxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoopProxy").finish_non_exhaustive()
    }
}

impl std::fmt::Debug for LoopReceiver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoopReceiver").finish_non_exhaustive()
    }
}

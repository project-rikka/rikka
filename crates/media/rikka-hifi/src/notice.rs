use std::sync::mpsc::{Receiver, Sender, channel};

use futures_channel::oneshot;
use rikka_hifi_core::SharedPath;
use rikka_worker::EventNotifier;

pub(crate) enum Notice {
    Graph(SharedPath),
    SharedFailed(String),
    Lost(String),
    ReleaseRequested(oneshot::Sender<()>),
}

pub(crate) struct OutputNotice {
    pub(crate) output: u64,
    pub(crate) notice: Notice,
}

#[derive(Clone)]
pub(crate) struct Notifier {
    sender: Sender<OutputNotice>,
    wake: EventNotifier,
    output: u64,
}

impl Notifier {
    pub(crate) fn open(wake: EventNotifier) -> (Notifier, Receiver<OutputNotice>) {
        let (sender, notices) = channel();
        (
            Notifier {
                sender,
                wake,
                output: 0,
            },
            notices,
        )
    }

    pub(crate) fn for_output(&self, output: u64) -> Notifier {
        Notifier {
            output,
            ..self.clone()
        }
    }

    pub(crate) fn send(&self, notice: Notice) {
        let output_notice = OutputNotice {
            output: self.output,
            notice,
        };
        if self.sender.send(output_notice).is_err() {
            return;
        }
        if let Err(error) = self.wake.notify() {
            tracing::debug!("failed to wake hifi engine: {error}");
        }
    }

    pub(crate) fn wake(&self) -> &EventNotifier {
        &self.wake
    }
}

impl std::fmt::Debug for Notifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Notifier")
            .field("output", &self.output)
            .finish_non_exhaustive()
    }
}

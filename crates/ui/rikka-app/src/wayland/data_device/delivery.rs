use std::sync::Arc;
use std::time::Duration;

use rikka_types::DragActions;
use rikka_worker::StreamLimit;

use crate::wayland::data_device::error::{Error, Result};
use crate::wayland::data_device::worker::{Command, CommandSender};

const TRANSFER: Duration = Duration::from_secs(5);
const WHOLE_TRANSFER: StreamLimit = StreamLimit {
    time: Duration::from_secs(30),
    bytes: 256 * 1024 * 1024,
};

#[derive(Clone)]
pub(crate) struct Delivery {
    link: Arc<Link>,
}

struct Link {
    id: u64,
    mimes: Vec<String>,
    actions: DragActions,
    sender: Arc<CommandSender>,
}

impl Delivery {
    pub(crate) fn new(
        id: u64,
        mimes: Vec<String>,
        actions: DragActions,
        sender: Arc<CommandSender>,
    ) -> Self {
        Self {
            link: Arc::new(Link {
                id,
                mimes,
                actions,
                sender,
            }),
        }
    }

    #[must_use]
    pub(crate) fn mimes(&self) -> &[String] {
        &self.link.mimes
    }

    #[must_use]
    pub(crate) fn actions(&self) -> DragActions {
        self.link.actions
    }

    pub(crate) fn read(&self, mime: &str) -> Result<Vec<u8>> {
        if !self.link.mimes.iter().any(|offered| offered == mime) {
            return Err(Error::NotOffered(mime.to_owned()));
        }
        let stream = self.link.sender.request(
            |reply| Command::Receive {
                id: self.link.id,
                mime: mime.to_owned(),
                reply,
            },
            TRANSFER,
        )??;
        Ok(rikka_worker::read_stream(stream, WHOLE_TRANSFER)?)
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        if let Err(error) = self.sender.send(Command::Finish { id: self.id }) {
            tracing::debug!("failed to finish drop: {error}");
        }
    }
}

impl std::fmt::Debug for Delivery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Delivery")
            .field("id", &self.link.id)
            .field("mimes", &self.link.mimes)
            .field("actions", &self.link.actions)
            .finish_non_exhaustive()
    }
}

use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use rikka_clipboard::Clipboard;
use rikka_picture::SharedGpu;

use crate::command::{Command, Link};
use crate::error::{Error, Result};
use crate::event::{EventSender, Events};
use crate::server::{self, Setup};
use crate::socket::Private;
use crate::view::View;

const GPU_PATIENCE: Duration = Duration::from_secs(5);

#[derive(Clone, Debug)]
pub struct Nested(Arc<Inner>);

#[derive(Debug)]
struct Inner {
    private: Private,
    link: Link,
    event_sender: Arc<EventSender>,
    thread: Option<JoinHandle<()>>,
}

#[must_use]
#[derive(Debug, Default)]
pub struct NestedBuilder {
    gpu: Option<SharedGpu>,
    clipboard: Option<Clipboard>,
}

impl NestedBuilder {
    pub fn gpu(mut self, gpu: SharedGpu) -> NestedBuilder {
        self.gpu = Some(gpu);
        self
    }

    pub fn clipboard(mut self, clipboard: Clipboard) -> NestedBuilder {
        self.clipboard = Some(clipboard);
        self
    }

    pub fn start(self) -> Result<Nested> {
        let (private, listener) = Private::bind()?;
        let event_sender = Arc::new(EventSender::default());
        let (ready, answer) = mpsc::channel();
        let setup = Setup {
            listener,
            gpu: self.gpu,
            gpu_patience: GPU_PATIENCE,
            clipboard: self.clipboard,
            event_sender: Arc::clone(&event_sender),
            ready,
        };
        let thread = std::thread::Builder::new()
            .name("rikka-nested".into())
            .spawn(move || server::run(setup))
            .map_err(Error::Thread)?;
        let link = match answer.recv() {
            Ok(Ok(link)) => link,
            Ok(Err(reason)) => {
                let _ = thread.join();
                return Err(Error::Start(reason));
            }
            Err(_) => {
                let _ = thread.join();
                return Err(Error::Start("server thread exited during startup".into()));
            }
        };
        Ok(Nested(Arc::new(Inner {
            private,
            link,
            event_sender,
            thread: Some(thread),
        })))
    }
}

impl Nested {
    pub fn builder() -> NestedBuilder {
        NestedBuilder::default()
    }

    pub fn start() -> Result<Nested> {
        Nested::builder().start()
    }

    #[must_use]
    pub fn socket(&self) -> &Path {
        self.0.private.path()
    }

    #[must_use]
    pub fn view(&self, app_id: impl Into<String>) -> View {
        View::adopt(&self.0.link, app_id.into())
    }

    #[must_use]
    pub fn events(&self) -> Option<Events> {
        self.0.event_sender.take()
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        let _ = self.link.send(Command::Stop);
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            tracing::error!("nested display thread panicked");
        }
        self.event_sender.close();
    }
}

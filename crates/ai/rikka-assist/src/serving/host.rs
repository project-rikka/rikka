use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, SendError, Sender, SyncSender};
use std::thread::{self, JoinHandle};

use rikka_seam::net::SocketListener;
use rikka_seam::{Call, Dispatcher, Seam, SeamError};

use crate::error::{Error, Result};
use crate::protocol::{SOCKET_EXTENSION, folder};
use crate::serving::Shared;
use crate::serving::service::AssistService;

const WORKERS: usize = 2;

#[derive(Debug)]
pub(crate) struct Host {
    mail: Sender<Mail>,
    thread: Option<JoinHandle<()>>,
}

enum Mail {
    Call(Box<Call>),
    Stop,
}

struct Inbox(Sender<Mail>);

impl Dispatcher for Inbox {
    fn enqueue(&self, call: Call) {
        if let Err(SendError(Mail::Call(call))) = self.0.send(Mail::Call(Box::new(call))) {
            call.fail(SeamError::Disconnected);
        }
    }
}

impl Host {
    pub(crate) fn start(shared: Arc<Shared>) -> Result<Host> {
        let name = format!(
            "{}-{}.{SOCKET_EXTENSION}",
            shared.offering().kit().to_ascii_lowercase(),
            std::process::id()
        );
        let path = folder().ok_or(Error::Sessionless)?.join(name);

        let (mail, inbox) = mpsc::channel();
        let (ready, readied) = mpsc::sync_channel(1);
        let sending = mail.clone();
        let thread = thread::Builder::new()
            .name("rikka-assist".to_owned())
            .spawn(move || serve(&shared, path, sending, &inbox, &ready))?;

        match readied.recv() {
            Ok(Ok(())) => Ok(Host {
                mail,
                thread: Some(thread),
            }),
            Ok(Err(error)) => {
                let _ = thread.join();
                Err(error.into())
            }
            Err(_) => {
                let _ = thread.join();
                Err(io::Error::other("the tools stopped before they were offered").into())
            }
        }
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.mail.send(Mail::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn serve(
    shared: &Arc<Shared>,
    path: PathBuf,
    mail: Sender<Mail>,
    inbox: &Receiver<Mail>,
    ready: &SyncSender<io::Result<()>>,
) {
    let working = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(WORKERS)
        .thread_name("rikka-assist-work")
        .enable_all()
        .build()
    {
        Ok(working) => working,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };

    let mut seam = Seam::new(Inbox(mail));
    let service = AssistService::new(Arc::clone(shared), working.handle().clone());
    if let Err(error) = seam.register(service) {
        let _ = ready.send(Err(io::Error::other(error.to_string())));
        return;
    }
    let listener = match SocketListener::serve(path, seam.client()) {
        Ok(listener) => listener,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    let _ = ready.send(Ok(()));

    while let Ok(Mail::Call(call)) = inbox.recv() {
        seam.dispatch(&mut (), *call);
    }

    drop(listener);
    working.shutdown_background();
}

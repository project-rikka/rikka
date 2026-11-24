mod access;
mod controller;
mod error;
mod identity;
mod language;
mod media;
mod notices;
mod options;
mod pam;
mod server;
mod service;
mod status;
mod uploads;
mod viewer;

use std::process::ExitCode;
use std::sync::mpsc::{self, SendError, Sender};

use futures_util::{Stream, StreamExt};
use rikka_seam::net::{SocketClient, SocketListener, socket_path_beside};
use rikka_seam::{Call, Dispatcher, Seam, SeamError};
use rikka_settings::seam::Settings;
use rikka_settings::session::remote;

use crate::controller::{Controller, Message};
use crate::error::{Error, Result};
use crate::language::Language;
use crate::service::RemoteService;

const SOCKET: &str = "remote";
const IO_WORKERS: usize = 2;

fn main() -> ExitCode {
    rikka_heap::return_large_glibc_blocks();
    rikka_log::install("info");
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!("{error}");
            ExitCode::FAILURE
        }
    }
}

struct MessageDispatcher(Sender<Message>);

impl Dispatcher for MessageDispatcher {
    fn enqueue(&self, call: Call) {
        if let Err(SendError(Message::Call(call))) = self.0.send(Message::Call(Box::new(call))) {
            call.fail(SeamError::Disconnected);
        }
    }
}

fn run() -> Result<()> {
    let connection = SocketClient::from_env().map_err(Error::NoSession)?;
    let compositor = connection.client();
    let settings = Settings::with_seam(remote::schema(), compositor.clone())?;
    let (sender, receiver) = mpsc::channel();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(IO_WORKERS)
        .thread_name("rikka-remote-io")
        .enable_all()
        .build()
        .map_err(Error::Workers)?;
    if let Some(changes) = settings.changes() {
        runtime.spawn(forward(changes, sender.clone(), Message::Settings));
    }
    let ended = sender.clone();
    runtime.spawn(async move {
        connection.closed().await;
        if ended.send(Message::SessionEnded).is_err() {
            tracing::debug!("main loop already stopped");
        }
    });

    let mut language = Language::follow(compositor.clone(), runtime.handle(), sender.clone());
    let mut controller = Controller::new(
        settings,
        compositor.clone(),
        runtime.handle().clone(),
        sender.clone(),
    );
    let mut seam = Seam::new(MessageDispatcher(sender));
    let context = seam.register(RemoteService)?;
    controller.publish_with(context.publisher().clone());
    let path = socket_path_beside(SOCKET).ok_or(Error::NoFolder)?;
    let _listener = SocketListener::serve(&path, seam.client()).map_err(Error::Workers)?;
    tracing::info!("listening on {}", path.display());

    while let Ok(message) = receiver.recv() {
        match message {
            Message::Call(call) => seam.dispatch(&mut controller, *call),
            Message::Settings(announcement) => controller.settings_changed(announcement),
            Message::Language(announcement) => language.changed(announcement),
            Message::Status(change) => controller.status_changed(change),
            Message::SessionEnded => {
                tracing::info!("session ended");
                break;
            }
        }
    }
    controller.stop();
    runtime.shutdown_background();
    Ok(())
}

async fn forward<T>(
    stream: impl Stream<Item = T>,
    sender: Sender<Message>,
    wrap: fn(T) -> Message,
) {
    let mut stream = std::pin::pin!(stream);
    while let Some(item) = stream.next().await {
        if sender.send(wrap(item)).is_err() {
            return;
        }
    }
}

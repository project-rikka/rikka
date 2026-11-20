mod clock;
mod controller;
mod engine;
mod error;
mod folder;
mod options;
mod saving;
mod service;
mod sound;
mod store;

use std::process::ExitCode;
use std::sync::mpsc::{self, SendError, Sender};

use futures_util::{Stream, StreamExt};
use rikka_seam::net::{SocketClient, SocketListener, socket_path_beside};
use rikka_seam::{Call, Dispatcher, Seam, SeamError};
use rikka_session::toplevels;
use rikka_settings::seam::Settings;
use rikka_settings::session::replay;

use crate::controller::{Controller, Message};
use crate::error::{Error, Result};
use crate::service::ReplayService;

const SOCKET: &str = "replay";

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
    let settings = Settings::with_seam(replay::schema(), compositor.clone())?;
    let (sender, receiver) = mpsc::channel();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .thread_name("rikka-replay-follow")
        .enable_all()
        .build()
        .map_err(Error::Workers)?;
    if let Some(changes) = settings.changes() {
        runtime.spawn(forward(changes, sender.clone(), Message::Settings));
    }
    let states = toplevels::states(&compositor);
    let ended_sender = sender.clone();
    runtime.spawn(async move {
        forward(states, ended_sender.clone(), Message::Toplevels).await;
        if ended_sender.send(Message::SessionEnded).is_err() {
            tracing::debug!("main loop already stopped");
        }
    });

    let mut controller = Controller::new(settings, sender.clone());
    let mut seam = Seam::new(MessageDispatcher(sender));
    let context = seam.register(ReplayService)?;
    controller.publish_with(context.publisher().clone());
    let path = socket_path_beside(SOCKET).ok_or(Error::NoFolder)?;
    let _listener = SocketListener::serve(&path, seam.client()).map_err(Error::Workers)?;
    tracing::info!("listening on {}", path.display());

    while let Ok(message) = receiver.recv() {
        match message {
            Message::Call(call) => seam.dispatch(&mut controller, *call),
            Message::Settings(announcement) => controller.settings_changed(announcement),
            Message::Toplevels(state) => controller.toplevels(state),
            Message::StateChanged => controller.publish_status(),
            Message::SessionEnded => {
                tracing::info!("session ended");
                break;
            }
        }
    }
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

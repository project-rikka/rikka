use std::time::Duration;

use rikka_remote_link::{LinkError, read_message};
use rikka_remote_wire::{ClientMessage, DecodeError, ServerMessage};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

const FLUSH_PATIENCE: Duration = Duration::from_secs(2);

#[derive(Clone, Debug)]
pub(crate) struct ControlWriter(mpsc::UnboundedSender<ServerMessage>);

pub(crate) struct ControlFlush(JoinHandle<()>);

impl ControlWriter {
    pub(crate) fn spawn(mut stream: quinn::SendStream) -> (ControlWriter, ControlFlush) {
        let (messages, mut queued) = mpsc::unbounded_channel::<ServerMessage>();
        let task = tokio::spawn(async move {
            while let Some(message) = queued.recv().await {
                if let Err(error) = stream.write_all(&message.encode()).await {
                    tracing::debug!("the control stream closed: {error}");
                    return;
                }
            }
            if let Err(error) = stream.finish() {
                tracing::debug!("cannot finish the control stream: {error}");
            }
        });
        (ControlWriter(messages), ControlFlush(task))
    }

    pub(crate) fn send(&self, message: ServerMessage) {
        if self.0.send(message).is_err() {
            tracing::debug!("a control message after the stream closed");
        }
    }
}

impl ControlFlush {
    pub(crate) async fn wait(self) {
        if tokio::time::timeout(FLUSH_PATIENCE, self.0).await.is_err() {
            tracing::debug!("the last control messages did not leave in time");
        }
    }
}

pub(crate) async fn next(recv: &mut quinn::RecvStream) -> Result<Option<ClientMessage>, LinkError> {
    loop {
        let Some((kind, body)) = read_message(recv).await? else {
            return Ok(None);
        };
        match ClientMessage::decode(kind, &body) {
            Ok(message) => return Ok(Some(message)),
            Err(DecodeError::UnknownType(kind)) => {
                tracing::debug!(kind, "skipped an unknown control message");
            }
            Err(error) => return Err(error.into()),
        }
    }
}

pub(crate) fn forward(
    mut recv: quinn::RecvStream,
) -> (mpsc::UnboundedReceiver<ClientMessage>, JoinHandle<()>) {
    let (messages, received) = mpsc::unbounded_channel();
    let task = tokio::spawn(async move {
        loop {
            match next(&mut recv).await {
                Ok(Some(message)) => {
                    if messages.send(message).is_err() {
                        return;
                    }
                }
                Ok(None) => return,
                Err(error) => {
                    tracing::debug!("the control stream broke: {error}");
                    return;
                }
            }
        }
    });
    (received, task)
}

use rikka_remote_wire::{ClientDatagram, Feedback};
use tokio::sync::mpsc;

use crate::media::{InputCommand, InputHandle};

pub(crate) async fn read(
    connection: quinn::Connection,
    input: InputHandle,
    feedback: mpsc::UnboundedSender<Feedback>,
) {
    let mut latest: Option<u16> = None;
    loop {
        let datagram = match connection.read_datagram().await {
            Ok(datagram) => datagram,
            Err(error) => {
                tracing::debug!("no more datagrams from the viewer: {error}");
                return;
            }
        };
        match ClientDatagram::decode(&datagram) {
            Ok(ClientDatagram::Motion(motion)) => {
                let fresh = latest.is_none_or(|known| {
                    motion.seq != known && motion.seq.wrapping_sub(known) < 0x8000
                });
                if fresh {
                    latest = Some(motion.seq);
                    input.send(InputCommand::Move(motion.at));
                }
            }
            Ok(ClientDatagram::Feedback(report)) => {
                if feedback.send(report).is_err() {
                    return;
                }
            }
            Err(error) => tracing::debug!("a malformed datagram from the viewer: {error}"),
        }
    }
}

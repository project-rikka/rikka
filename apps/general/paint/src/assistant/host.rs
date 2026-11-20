use std::time::Duration;

use paint_assistant::{Order, ToolHost, ToolResponse};
use rikka_app::Sender;
use tokio::sync::oneshot;

use crate::paint::Paint;

const REPLY_TIMEOUT: Duration = Duration::from_secs(180);

pub(crate) struct WindowToolHost {
    sender: Sender<Paint>,
}

impl WindowToolHost {
    pub(crate) fn new(sender: Sender<Paint>) -> Self {
        Self { sender }
    }
}

impl ToolHost for WindowToolHost {
    fn handle(&self, order: Order) -> ToolResponse {
        let (answer, reply) = oneshot::channel();
        let sent = self.sender.send(move |paint: &mut Paint| {
            paint.serve(order, answer);
        });
        Box::pin(async move {
            sent.map_err(|_| "the Paint window has closed".to_owned())?;
            match tokio::time::timeout(REPLY_TIMEOUT, reply).await {
                Ok(Ok(result)) => result,
                Ok(Err(_)) => Err("the Paint window closed before it answered".to_owned()),
                Err(_) => Err(
                    "Paint did not answer in time; the document may have moved to another window"
                        .to_owned(),
                ),
            }
        })
    }
}

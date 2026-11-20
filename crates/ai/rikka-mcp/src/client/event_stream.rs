use std::sync::Arc;
use std::time::Duration;

use rikka_net::{EventDecoder, Response};

use crate::client::http_link::{Shared, is_event_stream};
use crate::client::link::{Arrival, LinkError};
use crate::protocol::{Message, RequestId};

const MESSAGE: &str = "message";

const RECONNECTION_TIME: Duration = Duration::from_secs(1);
const RECONNECTION_CEILING: Duration = Duration::from_secs(60);
const RECONNECTION_DOUBLINGS: u32 = 6;
const FRUITLESS_RESUMPTIONS: u32 = 5;

#[derive(Default)]
struct Reader {
    last_event_id: Option<String>,
    reconnection_time: Option<Duration>,
    fruitless: u32,
}

enum Ended {
    Answered,
    Closed,
    Gone,
}

pub(super) async fn answer(shared: Arc<Shared>, mut response: Response, awaited: RequestId) {
    let mut reader = Reader::default();
    let reason = loop {
        match reader.read(&shared, response, Some(&awaited)).await {
            Ended::Answered | Ended::Gone => return,
            Ended::Closed => {}
        }
        if reader.last_event_id.is_none() {
            break "the event stream ended before the answer";
        }
        if reader.fruitless == FRUITLESS_RESUMPTIONS {
            break "the event stream kept ending before the answer";
        }

        reader.fruitless += 1;
        if !shared.paused(reader.pause()).await {
            return;
        }
        response = match shared.stream(reader.last_event_id.as_deref()).await {
            Ok(resumed) => resumed,
            Err(LinkError::Http(error)) if error.is_cancelled() => return,
            Err(error) => {
                tracing::debug!("failed to resume an event stream: {error}");
                break "the event stream could not be resumed";
            }
        };
    };

    shared
        .arrive(Arrival::Lost {
            request: awaited,
            reason: reason.to_owned(),
        })
        .await;
}

pub(super) async fn listen(shared: Arc<Shared>, listener: u64) {
    let mut reader = Reader::default();
    while shared.is_listening(listener) {
        match shared.stream(reader.last_event_id.as_deref()).await {
            Ok(response) => {
                if matches!(reader.read(&shared, response, None).await, Ended::Gone) {
                    return;
                }
            }
            Err(LinkError::Http(error)) if error.is_transient() => {
                tracing::debug!("failed to listen to an MCP server: {error}");
            }
            Err(error) => {
                tracing::debug!("an MCP server is not listened to: {error}");
                return;
            }
        }

        reader.fruitless += 1;
        if !shared.paused(reader.pause()).await {
            return;
        }
    }
}

pub(super) async fn first_message(mut response: Response) -> Result<(), LinkError> {
    if !is_event_stream(&response) {
        response.bytes().await?;
        return Ok(());
    }

    let mut decoder = EventDecoder::new();
    while let Some(chunk) = response.chunk().await? {
        decoder.push(&chunk);
        while let Some(event) = decoder.next_event() {
            if event.event() == MESSAGE && !event.data().is_empty() {
                return Ok(());
            }
        }
    }
    Ok(())
}

impl Reader {
    async fn read(
        &mut self,
        shared: &Shared,
        mut response: Response,
        awaited: Option<&RequestId>,
    ) -> Ended {
        let mut decoder = EventDecoder::new();
        loop {
            let chunk = match response.chunk().await {
                Ok(Some(chunk)) => chunk,
                Ok(None) => return Ended::Closed,
                Err(error) if error.is_cancelled() => return Ended::Gone,
                Err(error) => {
                    tracing::debug!("an event stream broke: {error}");
                    return Ended::Closed;
                }
            };

            decoder.push(&chunk);
            if let Some(reconnection_time) = decoder.reconnection_time() {
                self.reconnection_time = Some(reconnection_time);
            }
            if let Some(id) = decoder.last_event_id()
                && self.last_event_id.as_deref() != Some(id)
            {
                self.last_event_id = Some(id.to_owned());
                self.fruitless = 0;
            }
            while let Some(event) = decoder.next_event() {
                if event.event() != MESSAGE || event.data().is_empty() {
                    continue;
                }
                let message: Message = match serde_json::from_str(event.data()) {
                    Ok(message) => message,
                    Err(error) => {
                        tracing::debug!("skipping an event that is no MCP message: {error}");
                        continue;
                    }
                };

                self.fruitless = 0;
                let answered = awaited.is_some_and(|awaited| message.answers(awaited));
                if !shared.arrive(Arrival::Message(message)).await {
                    return Ended::Gone;
                }
                if answered {
                    return Ended::Answered;
                }
            }
        }
    }

    fn pause(&self) -> Duration {
        let doublings = self.fruitless.saturating_sub(1).min(RECONNECTION_DOUBLINGS);
        self.reconnection_time
            .unwrap_or(RECONNECTION_TIME)
            .saturating_mul(1 << doublings)
            .min(RECONNECTION_CEILING)
    }
}

use std::pin::pin;
use std::time::Duration;

use futures_util::future::{Either, select};
use futures_util::{StreamExt, stream};
use music_core::{Error, Result};
use rikka_net::{Bytes, Client, Response, header};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use crate::plan::{Piece, Plan};
use crate::spool::Spool;

const CONNECTING: Duration = Duration::from_secs(10);
const STALLED: Duration = Duration::from_secs(30);
const PARTIAL: u16 = 206;
const PAST_THE_END: u16 = 416;

#[derive(Clone, Debug)]
pub struct Fetcher {
    client: Client,
    ahead: usize,
    tries: u32,
}

enum Arrival {
    Bytes(Bytes),
    Ended,
    Failed(Error),
}

enum Cut {
    Network(Error),
    Fatal(Error),
}

struct Reading {
    got: u64,
    length: Option<u64>,
    framed: bool,
}

impl Fetcher {
    pub fn new(agent: &str) -> Fetcher {
        Fetcher {
            client: Client::builder()
                .user_agent(agent)
                .connect_timeout(CONNECTING)
                .read_timeout(STALLED)
                .build()
                .unwrap_or_default(),
            ahead: 1,
            tries: 1,
        }
    }

    #[must_use]
    pub fn ahead(mut self, pieces: usize) -> Fetcher {
        self.ahead = pieces.max(1);
        self
    }

    #[must_use]
    pub fn tries(mut self, tries: u32) -> Fetcher {
        self.tries = tries.max(1);
        self
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub async fn fetch(&self, plan: Plan, spool: &mut Spool) -> Result<()> {
        spool.begin(plan.total());
        spool.write(&plan.lead)?;
        let plan = &plan;
        let (queue, mut queued) = unbounded_channel();
        let producing = async move {
            stream::iter(plan.pieces.iter().cloned())
                .map(|piece| {
                    let (arrivals, arriving) = unbounded_channel();
                    let _ = queue.send(arriving);
                    async move { self.piece(plan, &piece, arrivals).await }
                })
                .buffered(self.ahead)
                .for_each(|()| async {})
                .await;
            drop(queue);
        };
        let consuming = async {
            while let Some(mut arriving) = queued.recv().await {
                while let Some(arrival) = arriving.recv().await {
                    match arrival {
                        Arrival::Bytes(bytes) => spool.write(&bytes)?,
                        Arrival::Ended => return Ok(()),
                        Arrival::Failed(error) => return Err(error),
                    }
                }
            }
            Ok(())
        };
        match select(pin!(producing), pin!(consuming)).await {
            Either::Left(((), consuming)) => consuming.await,
            Either::Right((consumed, _)) => consumed,
        }
    }

    async fn piece(&self, plan: &Plan, piece: &Piece, arrivals: UnboundedSender<Arrival>) {
        let mut reading = Reading {
            got: 0,
            length: piece.length,
            framed: false,
        };
        let mut tries = 1;
        let ended = loop {
            match self.attempt(plan, piece, &mut reading, &arrivals).await {
                Ok(ended) => break ended,
                Err(Cut::Network(error)) if tries < self.tries => {
                    tracing::debug!("resuming {} from byte {}: {error}", piece.url, reading.got);
                    tries += 1;
                }
                Err(Cut::Network(error) | Cut::Fatal(error)) => {
                    let _ = arrivals.send(Arrival::Failed(error));
                    return;
                }
            }
        };
        if ended {
            let _ = arrivals.send(Arrival::Ended);
        }
    }

    async fn attempt(
        &self,
        plan: &Plan,
        piece: &Piece,
        reading: &mut Reading,
        arrivals: &UnboundedSender<Arrival>,
    ) -> std::result::Result<bool, Cut> {
        let start = piece.from + reading.got;
        let ranged = piece.ranged || reading.got > 0;
        let mut request = self.client.get(piece.url.as_str());
        for (name, value) in &plan.headers {
            request = request.header(*name, value.as_str());
        }
        if ranged {
            let end = reading
                .length
                .map(|length| (piece.from + length - 1).to_string())
                .unwrap_or_default();
            request = request.header(header::RANGE, format!("bytes={start}-{end}"));
        }
        let mut response = request
            .send()
            .await
            .map_err(|error| Cut::Network(Error::unreachable(error)))?;
        let status = response.status().as_u16();
        if status == PAST_THE_END && plan.open {
            return Ok(true);
        }
        if !response.status().is_success() {
            let refusal =
                Error::from_status(status, format!("{} answered {status}", host(&response)));
            return Err(match refusal {
                Error::Unreachable(_) => Cut::Network(refusal),
                _ => Cut::Fatal(refusal),
            });
        }
        let whole = ranged && status != PARTIAL;
        let length = match reading.length {
            Some(length) => length,
            None => response.content_length().ok_or_else(|| {
                Cut::Fatal(Error::Unreadable(format!(
                    "{} sent a piece without its length",
                    host(&response)
                )))
            })?,
        };
        reading.length = Some(length);
        if !reading.framed
            && let Some(frame) = &plan.frame
        {
            let framing = frame(length).map_err(Cut::Fatal)?;
            let _ = arrivals.send(Arrival::Bytes(framing.into()));
            reading.framed = true;
        }
        let mut skip = if whole { start } else { 0 };
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| Cut::Network(Error::unreachable(error)))?
        {
            let dropped = skip.min(chunk.len() as u64);
            skip -= dropped;
            let mut fresh = chunk.slice(dropped as usize..);
            let room = length - reading.got;
            if fresh.len() as u64 > room {
                if !whole {
                    return Err(Cut::Fatal(Error::Unreadable(format!(
                        "{} sent more than the {length} bytes requested",
                        host(&response)
                    ))));
                }
                fresh.truncate(room as usize);
            }
            reading.got += fresh.len() as u64;
            if !fresh.is_empty() {
                let _ = arrivals.send(Arrival::Bytes(fresh));
            }
            if reading.got == length {
                return Ok(false);
            }
        }
        match reading.got == length || plan.open {
            true => Ok(reading.got < length),
            false => Err(Cut::Network(Error::Unreachable(format!(
                "{} ended after {} of {length} bytes",
                host(&response),
                reading.got
            )))),
        }
    }
}

fn host(response: &Response) -> &str {
    response.url().host_str().unwrap_or("the stream host")
}

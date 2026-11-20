use std::time::{Duration, Instant};

use async_io::Timer;
use futures_channel::mpsc::{self, UnboundedSender};
use futures_util::StreamExt as _;
use futures_util::future::{Either, select};
use rikka_boost::service::{MAX_HOLD, OBJECT, SERVICE};
use zbus::connection::Builder;
use zbus::interface;

use crate::clocks::Clocks;
use crate::error::{Error, Result};

struct Boost {
    holds: UnboundedSender<Duration>,
}

#[interface(name = "os.rikka.Boost1")]
impl Boost {
    fn hold(&self, milliseconds: u32) {
        let duration = Duration::from_millis(milliseconds.into()).min(MAX_HOLD);
        self.holds
            .unbounded_send(duration)
            .expect("the clock loop outlives the D-Bus connection");
    }
}

pub(crate) async fn run(mut clocks: Clocks) -> Result<()> {
    let (sender, mut holds) = mpsc::unbounded();
    let _connection = Builder::system()
        .map_err(Error::Bus)?
        .name(SERVICE)
        .and_then(|builder| builder.serve_at(OBJECT, Boost { holds: sender }))
        .map_err(Error::NameRequest)?
        .build()
        .await
        .map_err(Error::NameRequest)?;

    let mut expires_at: Option<Instant> = None;
    loop {
        let expiry = match expires_at {
            Some(at) => Timer::at(at),
            None => Timer::never(),
        };
        match select(holds.next(), expiry).await {
            Either::Left((Some(duration), _)) if duration.is_zero() => {}
            Either::Left((Some(duration), _)) => {
                let at = Instant::now() + duration;
                if expires_at.is_none() {
                    clocks.raise();
                }
                expires_at = Some(expires_at.map_or(at, |current| current.max(at)));
            }
            Either::Left((None, _)) => break,
            Either::Right(_) => {
                clocks.lower();
                expires_at = None;
            }
        }
    }
    clocks.lower();
    Ok(())
}

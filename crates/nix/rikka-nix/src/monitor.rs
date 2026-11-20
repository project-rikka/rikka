use std::sync::Arc;

use rikka_bus::Heard;
use rikka_nix_protocol::{PageRecord, Run, RunId, RunRecord};
use rikka_sync::Mutex;
use zbus::blocking::{Connection, Proxy};

use crate::bus::{self, Follower};
use crate::error::Result;
use crate::event::Event;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    pub run: RunId,
    pub from: u64,
    pub lines: Vec<String>,
    pub next: u64,
    pub end: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interest {
    Progress,
    Transcript,
}

impl Interest {
    fn members(self) -> &'static [&'static str] {
        match self {
            Self::Progress => &[bus::STARTED, bus::STATUS, bus::FINISHED],
            Self::Transcript => &[bus::STARTED, bus::STATUS, bus::FINISHED, bus::LINES],
        }
    }
}

#[derive(Clone, Debug)]
pub struct Monitor(Arc<Watch>);

#[derive(Debug)]
struct Watch {
    _signals: rikka_bus::Watch,
    connection: Connection,
    proxy: Proxy<'static>,
}

impl Monitor {
    pub fn watch(interest: Interest, report: impl Fn(Event) + Send + 'static) -> Result<Self> {
        let connection = bus::connect()?;
        let follower = Arc::new(Mutex::new(Follower::new(None)));
        let reading = Arc::clone(&follower);
        let signals = bus::signals(interest.members())
            .hear(
                &connection,
                move |message| reading.lock().read(message),
                move |heard| {
                    report(match heard {
                        Heard::Signal(event) => event,
                        Heard::Closed => Event::Disconnected,
                    });
                    true
                },
            )
            .map_err(|error| bus::unsubscribed(&error))?;
        follower.lock().owned_by(bus::owner(&connection)?);
        let proxy = bus::proxy(&connection)?;
        Ok(Self(Arc::new(Watch {
            _signals: signals,
            connection,
            proxy,
        })))
    }

    pub fn runs(&self) -> Result<Vec<Run>> {
        let records: Vec<RunRecord> = self
            .0
            .proxy
            .call("Runs", &())
            .map_err(|error| bus::unavailable(&error))?;
        records
            .into_iter()
            .map(|record| Ok(Run::try_from(record)?))
            .collect()
    }

    pub fn running(&self) -> Result<Option<Run>> {
        if bus::owner(&self.0.connection)?.is_none() {
            return Ok(None);
        }
        Ok(self.runs()?.into_iter().find(Run::is_running))
    }

    pub fn transcript(&self, run: RunId, from: u64) -> Result<Page> {
        let page: PageRecord = self
            .0
            .proxy
            .call("Transcript", &(run.get(), from))
            .map_err(|error| bus::unavailable(&error))?;
        Ok(Page {
            run,
            from,
            lines: page.lines,
            next: page.next,
            end: page.end,
        })
    }
}

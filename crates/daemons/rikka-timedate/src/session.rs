use zbus::blocking::Connection;
use zbus::proxy::CacheProperties;

use crate::proxies::{
    TIMEDATE, TIMEDATE_PATH, TIMESYNC, TIMESYNC_MANAGER, TIMESYNC_PATH, TimedateProxy,
};
use crate::state::{Action, Event, State};
use rikka_bus::{
    Daemon, EventSink, Fault, Handler, Notifier, Properties, Rule, Signals, StartError, Watch,
};

pub(crate) struct Timedated {
    connection: Connection,
    timedate: TimedateProxy<'static>,
    _signals: Option<Watch>,
}

pub(crate) enum Request {
    SetTime(i64),
    Reread,
}

impl Handler for Timedated {
    type Config = ();
    type Request = Request;
    type Notice = ();
    type Event = Event;
    type State = State;

    const THREAD_NAME: &'static str = "rikka-timedate";
    const UNAVAILABLE: &'static str = "this machine does not tell its clock";

    fn open(
        (): (),
        notifier: Notifier<Timedated>,
        report: &EventSink<Event>,
    ) -> Result<Timedated, StartError> {
        let connection = Connection::system()?;
        let timedate = TimedateProxy::builder(&connection)
            .cache_properties(CacheProperties::No)
            .build()?;
        let signals = watch(&connection, notifier)
            .inspect_err(|error| tracing::warn!("failed to watch clock signals: {error}"))
            .ok();
        let timedated = Timedated {
            connection,
            timedate,
            _signals: signals,
        };
        report.emit(Event::Read(timedated.read()));
        Ok(timedated)
    }

    fn handle_request(&mut self, request: Request, report: &EventSink<Event>) {
        match request {
            Request::SetTime(micros) => {
                let answered = self
                    .timedate
                    .set_time(micros, false, true)
                    .map_err(|error| Fault::from_zbus(Daemon::Timedated, &error))
                    .err()
                    .map(|error| {
                        tracing::warn!("failed to set the clock: {error}");
                        error.to_string()
                    });
                report.emit(Event::Answered {
                    action: Action::SetTime,
                    error: answered,
                });
                report.emit(Event::Read(self.read()));
            }
            Request::Reread => report.emit(Event::Read(self.read())),
        }
    }

    fn handle_notice(&mut self, (): (), report: &EventSink<Event>) {
        report.emit(Event::Read(self.read()));
    }
}

impl Timedated {
    fn read(&self) -> State {
        let clock = Properties::read(&self.connection, TIMEDATE, TIMEDATE_PATH, TIMEDATE)
            .inspect_err(|error| tracing::debug!("{TIMEDATE} is not responding: {error}"))
            .unwrap_or_default();
        let sync = Properties::read(&self.connection, TIMESYNC, TIMESYNC_PATH, TIMESYNC_MANAGER)
            .inspect_err(|error| tracing::debug!("{TIMESYNC} is not responding: {error}"))
            .unwrap_or_default();
        State {
            zone: clock
                .string_ref("Timezone")
                .filter(|zone| !zone.is_empty())
                .map(str::to_owned),
            local_rtc: clock.boolean("LocalRTC").unwrap_or(false),
            can_ntp: clock.boolean("CanNTP").unwrap_or(false),
            ntp: clock.boolean("NTP").unwrap_or(false),
            synchronized: clock.boolean("NTPSynchronized").unwrap_or(false),
            server: sync
                .string_ref("ServerName")
                .filter(|server| !server.is_empty())
                .map(str::to_owned),
            error: None,
        }
    }
}

fn watch(connection: &Connection, notifier: Notifier<Timedated>) -> zbus::Result<Watch> {
    Signals::new()
        .rule(Rule::new().sender(TIMEDATE).path(TIMEDATE_PATH))
        .rule(Rule::new().sender(TIMESYNC).path(TIMESYNC_PATH))
        .listen(connection, |_| Some(()), move |()| notifier.notify(()))
}

impl std::fmt::Debug for Timedated {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Timedated").finish_non_exhaustive()
    }
}
